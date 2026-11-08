//! Editor state and operations, independent of GPUI: selection,
//! dragging (root move / FK aim / two-bone IK), frame operations,
//! undo snapshots, save/load and export triggers. The GPUI layer in
//! `app.rs` only translates mouse events and buttons into these calls.

use crate::export::{
    export_gif as render_gif, export_mp4 as render_mp4, export_png_sequence,
    export_spritesheet as render_sheet, export_webm as render_webm,
};
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

/// Wave 7: what a draw-mode drag does — paint a stroke with the
/// active brush preset, or erase stroke points around the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTool {
    Brush,
    Eraser,
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
    redo: Vec<String>,
    /// Wave 9: edits counted at `snapshot()` (the undo choke
    /// point) over this core's lifetime. A few appearance
    /// cyclers (segment colour/shape, figure opacity) mutate
    /// without snapshotting by design and are NOT counted —
    /// autosave is driven by undoable edits.
    edit_count: u64,
    /// Wave 9: `edit_count` at the last successful autosave.
    last_autosave_count: u64,
    clipboard: Option<crate::model::Frame>,
    /// Wave 9: deep-copied frames held by `copy_range` for
    /// `paste_range`. Distinct from the single-frame `clipboard`.
    frame_clipboard: Vec<crate::model::Frame>,
    pub playhead: f32,
    /// Wave 7: inclusive frame range the playhead loops inside,
    /// when set. Runtime only, like `solo`: undo snapshots cover
    /// the project, not this.
    pub loop_range: Option<(usize, usize)>,
    pub show_paths: bool,
    pub light_theme: bool,
    pub library: Vec<(String, FigureDef)>,
    pub library_index: usize,
    /// Draw mode: canvas drags paint brush strokes on this frame.
    pub draw_mode: bool,
    drawing: bool,
    /// Wave 7: the active draw tool (brush or eraser).
    pub draw_tool: DrawTool,
    /// Wave 7: index into `paint::brush_library` — the active
    /// brush preset (also sizes the eraser).
    pub brush_index: usize,
    /// Strokes changed so far by the current eraser drag, for the
    /// status line when the drag ends.
    erase_changed: usize,
    /// How many previous frames the onion skin shows (1..=3).
    pub onion_frames: u8,
    pub project_files: Vec<PathBuf>,
    pub project_index: usize,
    /// UI language for toolbar labels (see i18n.rs).
    pub lang: crate::i18n::Lang,
    /// Active 3D editing session (fig3d), if any. Runtime only:
    /// undo snapshots cover the project, not this state.
    pub fig3: Option<crate::fig3d::Fig3State>,
    /// Wave 5: solo figure (layers) — display/edit focus.
    pub solo: Option<usize>,
    /// Wave 7: active movieclip editing session, if any.
    pub clip_edit: Option<ClipEditSession>,
}

/// Runtime state of a movieclip editing session (wave 7): which
/// clip is being edited and which of its frames is current.
/// Runtime only — undo snapshots cover the project, not this
/// state (same convention as `fig3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipEditSession {
    pub clip: usize,
    pub frame: usize,
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
            redo: Vec::new(),
            edit_count: 0,
            last_autosave_count: 0,
            clipboard: None,
            frame_clipboard: Vec::new(),
            playhead: 0.0,
            loop_range: None,
            show_paths: false,
            light_theme: false,
            library: Self::load_library(),
            library_index: 0,
            draw_mode: false,
            drawing: false,
            draw_tool: DrawTool::Brush,
            brush_index: 0,
            erase_changed: 0,
            onion_frames: 1,
            project_files: Self::list_project_files(),
            project_index: 0,
            lang: crate::i18n::Lang::En,
            fig3: None,
            solo: None,
            clip_edit: None,
        }
    }

    fn list_project_files() -> Vec<PathBuf> {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("projects");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                    // The shell keeps its own files in this
                    // directory (layout prefs, recovery snapshot,
                    // recents); they are not openable projects
                    // and must never surface in the selector —
                    // the stray ".layout" row (Phase F fix).
                    .filter(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        !name.starts_with('.')
                            && name != crate::shell::LAYOUT_FILE
                            && name != crate::shell::RECOVERY_FILE
                            && name != crate::shell::RECENTS_FILE
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    /// The local figure library: JSON figure definitions from
    /// `./library/`, plus three built-ins so it is never empty.
    fn load_library() -> Vec<(String, FigureDef)> {
        let mut lib: Vec<(String, FigureDef)> = vec![
            ("Built-in: Humanoid".into(), FigureDef::humanoid()),
            ("Built-in: Quadruped".into(), FigureDef::quadruped()),
            ("Built-in: Mini".into(), FigureDef::simple()),
        ];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                .collect();
            files.sort();
            for f in files {
                if let Ok(text) = std::fs::read_to_string(&f) {
                    if let Ok(def) = serde_json::from_str::<FigureDef>(&text) {
                        let name = f
                            .file_stem()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Figure".into());
                        lib.push((name, def));
                    }
                }
            }
        }
        lib
    }

    pub fn frame_count(&self) -> usize {
        self.project.frames.len()
    }

    /// Joint positions of figure `fi` in frame `frame`.
    pub fn positions(&self, frame: usize, fi: usize) -> Vec<V2> {
        let fig = &self.project.figures[fi];
        evaluate(&fig.def, &self.project.frames[frame].poses[fi])
    }

    pub fn snapshot(&mut self) {
        if let Ok(s) = serde_json::to_string(&self.project) {
            self.undo.push(s);
            if self.undo.len() > 60 {
                self.undo.remove(0);
            }
            self.redo.clear();
            // Wave 9: every recorded undo step is one counted
            // edit for the autosave cadence.
            self.edit_count += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            if let Ok(current) = serde_json::to_string(&self.project) {
                self.undo.push(current);
            }
            if let Ok(p) = serde_json::from_str::<Project>(&s) {
                self.project = p;
                self.current = self.current.min(self.project.frames.len().saturating_sub(1));
                self.status = "Redone.".into();
            }
        } else {
            self.status = "Nothing to redo.".into();
        }
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            if let Ok(current) = serde_json::to_string(&self.project) {
                self.redo.push(current);
            }
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

    /// Number of edits that can currently be undone (the undo
    /// stack depth, capped at 60 by `snapshot`).
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of edits that can currently be redone.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
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
        if self.project.figures[fi].locked {
            self.status = "Figure is locked — unlock it in the layers list.".into();
            return false;
        }
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
        let duration = self.project.frames[self.current].duration;
        self.project
            .frames
            .insert(self.current + 1, crate::model::Frame { poses, duration, easing: None, bends: Vec::new() });
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
        self.project.frames.insert(
            self.current + 1,
            crate::model::Frame { poses, duration: 1.0, easing: None, bends: Vec::new() },
        );
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
            shape: crate::model::Shape::Line,
            style: crate::model::NodeStyle::default(),
            media: None,
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

    /// Cycle the selected segment's shape: line -> circle -> triangle
    /// -> square -> pentagon -> line.
    pub fn cycle_shape(&mut self) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        node.shape = match node.shape {
            crate::model::Shape::Line => crate::model::Shape::Circle,
            crate::model::Shape::Circle => crate::model::Shape::Polygon(3),
            crate::model::Shape::Polygon(n) if n < 6 => crate::model::Shape::Polygon(n + 1),
            crate::model::Shape::Polygon(_) => crate::model::Shape::Line,
        };
        self.status = format!("{} shape: {:?}.", node.name, node.shape);
    }

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

    /// Set the selected segment's colour directly (the colour
    /// picker panel). Writes the same `NodeDef.color` the cycle
    /// op steps through, as `Some(rgb)`; like the cycle op it
    /// does not snapshot.
    pub fn set_node_color(&mut self, rgb: u32) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        node.color = Some(rgb);
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

    /// Upsert a camera key at the current frame (exactly one
    /// snapshot), preserving the camera angle in effect at this
    /// frame: `set_camera_key` alone would give a fresh key
    /// angle 0, snapping any interpolated rotation — the same
    /// fix-up `rotate_camera` applies.
    fn set_camera_key_here(&mut self, x: f32, y: f32, zoom: f32) {
        self.snapshot();
        let frame = self.current;
        let angle = self.project.camera_angle_at(frame as f32);
        self.project.set_camera_key(frame, x, y, zoom);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = angle;
        }
    }

    /// Fit the camera to every visible figure at the current
    /// frame (the canvas Fit button): one camera key at the
    /// current frame, centred on the figures' joint bounds with a
    /// zoom that fits those bounds into the stage with a 10%
    /// margin, angle preserved.
    ///
    /// Camera convention (from the rasterizer's view closure in
    /// export.rs `render_frame_at`, mirrored by the canvas map in
    /// app.rs `paint_stage`): the key's (x, y) is the stage point
    /// shown at the centre of the view and a stage span of length
    /// L occupies L * zoom view units, so the view shows
    /// stage_w / zoom by stage_h / zoom stage units around the
    /// centre. Fitting bounds of bw x bh therefore needs
    /// zoom <= stage_w / bw and zoom <= stage_h / bh; the 10%
    /// margin divides that fit zoom by 1.1. A zero-size bounds
    /// dimension (every joint on one line) imposes no constraint,
    /// and a single-point bounds keeps the current zoom.
    pub fn frame_all_figures(&mut self) {
        let poses = self.project.poses_at(self.current as f32);
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        let mut any = false;
        for (fi, fig) in self.project.figures.iter().enumerate() {
            if fig.hidden {
                continue;
            }
            let Some(pose) = poses.get(fi) else { continue };
            for p in evaluate(&fig.def, pose) {
                any = true;
                min[0] = min[0].min(p[0]);
                min[1] = min[1].min(p[1]);
                max[0] = max[0].max(p[0]);
                max[1] = max[1].max(p[1]);
            }
        }
        if !any {
            self.status = "No figures to frame.".into();
            return;
        }
        let centre = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        let (bw, bh) = (max[0] - min[0], max[1] - min[1]);
        let mut fit = f32::INFINITY;
        if bw > 0.0 {
            fit = fit.min(self.project.stage[0] / bw);
        }
        if bh > 0.0 {
            fit = fit.min(self.project.stage[1] / bh);
        }
        let zoom = if fit.is_finite() {
            fit / 1.1
        } else {
            self.camera_here().2
        };
        self.set_camera_key_here(centre[0], centre[1], zoom);
        self.status = format!(
            "Camera fit all figures: zoom {zoom:.2} at frame {}.",
            self.current + 1
        );
    }

    /// Centre the camera on the selected figure's root at the
    /// current frame (the canvas Centre button): one camera key
    /// at the current frame with the current zoom and angle
    /// unchanged, only the centre moved to the root.
    pub fn centre_on_selection(&mut self) {
        if self.selected_figure >= self.project.figures.len() {
            self.status = "No figures to centre on.".into();
            return;
        }
        let poses_here = self.project.poses_at(self.current as f32);
        let Some(pose) = poses_here.get(self.selected_figure) else {
            self.status = "No figures to centre on.".into();
            return;
        };
        let root = pose.root;
        let zoom = self.camera_here().2;
        let name = self.project.figures[self.selected_figure].name.clone();
        self.set_camera_key_here(root[0], root[1], zoom);
        self.status = format!("Camera centred on {name} at frame {}.", self.current + 1);
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
            self.playhead = frame as f32;
        }
    }

    pub fn advance(&mut self) {
        if !self.project.frames.is_empty() {
            self.current = (self.current + 1) % self.project.frames.len();
            self.playhead = self.current as f32;
        }
    }

    /// Advance the playhead by wall-clock seconds, honouring fps and
    /// per-frame durations. Loops at the end — or, when a loop range
    /// is set, at the end of the range (see `set_loop_range`).
    pub fn advance_play(&mut self, dt_secs: f32) {
        let n = self.project.frames.len();
        if n == 0 {
            return;
        }
        let duration = self.project.frames[self.current].duration.max(0.05);
        self.playhead += dt_secs * self.project.fps.max(1.0) / duration;
        match self.loop_range {
            Some((a, b)) => {
                // Clamp defensively: frames may have been deleted
                // since the range was set.
                let b = b.min(n - 1);
                let a = a.min(b);
                // The range is inclusive, so the playhead wraps when
                // it reaches the end of frame b (playhead == b + 1),
                // stepping back by the range's span to a. A playhead
                // below a (e.g. after a `goto` outside the range)
                // never triggers this: it just plays forward into the
                // range, and the range itself is left untouched.
                let end = (b + 1) as f32;
                let span = end - a as f32;
                while self.playhead >= end {
                    self.playhead -= span;
                }
            }
            None => {
                while self.playhead >= n as f32 {
                    self.playhead -= n as f32;
                }
            }
        }
        self.current = (self.playhead.floor() as usize).min(n - 1);
    }

    /// Frame time the canvas should draw: fractional while playing
    /// with tween playback on, otherwise the authored frame.
    pub fn display_frame(&self) -> f32 {
        if self.playing && self.project.tween_playback {
            self.playhead
        } else {
            self.current as f32
        }
    }

    // ---- Wave 7: timeline depth --------------------------------------

    /// Set the inclusive loop range [a, b] the playhead wraps inside
    /// during playback (see `advance_play`). The endpoints are
    /// ordered and clamped to the timeline; a == b is a valid
    /// single-frame loop. This is editor runtime state, not project
    /// data, so it is not snapshotted for undo.
    pub fn set_loop_range(&mut self, a: usize, b: usize) {
        let n = self.project.frames.len();
        if n == 0 {
            self.loop_range = None;
            self.status = "No frames to loop.".into();
            return;
        }
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let b = b.min(n - 1);
        let a = a.min(b);
        self.loop_range = Some((a, b));
        self.status = format!("Looping frames {}–{}.", a + 1, b + 1);
    }

    pub fn clear_loop_range(&mut self) {
        self.loop_range = None;
        self.status = "Loop range cleared.".into();
    }

    /// Where an index lands after the block [from, from+count) is
    /// moved to start at `target`: indices inside the block move
    /// with it, indices the block jumps over shift by `count`
    /// against the move, everything else stays.
    fn remap_after_shift(idx: usize, from: usize, count: usize, target: usize) -> usize {
        if idx >= from && idx < from + count {
            idx - from + target
        } else if target > from && idx >= from + count && idx < target + count {
            idx - count
        } else if target < from && idx >= target && idx < from {
            idx + count
        } else {
            idx
        }
    }

    /// After a frame-block move, keep `current` and the loop range
    /// pointing at the same frames they pointed at before.
    fn remap_after_frame_move(&mut self, from: usize, count: usize, target: usize) {
        self.current = Self::remap_after_shift(self.current, from, count, target);
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let a = Self::remap_after_shift(a, from, count, target);
            let b = Self::remap_after_shift(b, from, count, target);
            self.loop_range = Some(if a <= b { (a, b) } else { (b, a) });
        }
    }

    /// Move the block of `count` frames starting at `from` by
    /// `delta` positions (negative = earlier, positive = later).
    /// Frames are values — poses, bends, durations and easings all
    /// travel with their frame automatically. Returns false and
    /// changes nothing when the block or its destination would fall
    /// outside the timeline (or the move is empty).
    pub fn shift_frames(&mut self, from: usize, count: usize, delta: i32) -> bool {
        let n = self.project.frames.len();
        if count == 0 || delta == 0 || from >= n || from + count > n {
            return false;
        }
        let target = from as i32 + delta;
        if target < 0 || target as usize + count > n {
            return false;
        }
        let target = target as usize;
        self.snapshot();
        // Drain the block, then re-insert it at `target`. The
        // insert index is the same in the drained vec for both
        // directions: exactly the frames that must precede the
        // block (everything before it originally, plus the frames
        // it jumps over) sit in front of `target` after the drain.
        let block: Vec<_> = self.project.frames.drain(from..from + count).collect();
        for (i, frame) in block.into_iter().enumerate() {
            self.project.frames.insert(target + i, frame);
        }
        self.remap_after_frame_move(from, count, target);
        self.status = format!(
            "Moved {count} frame(s) to position {}.",
            target + 1
        );
        true
    }

    /// Reverse the block of `count` frames starting at `from`, in
    /// place — poses, bends, durations and easings reverse with
    /// their frames. Out-of-range blocks are clamped; a block of
    /// fewer than two frames is a no-op.
    pub fn reverse_range(&mut self, from: usize, count: usize) {
        let n = self.project.frames.len();
        if from >= n {
            return;
        }
        let count = count.min(n - from);
        if count < 2 {
            return;
        }
        self.snapshot();
        self.project.frames[from..from + count].reverse();
        let last = from + count - 1;
        let flip = |idx: usize| {
            if idx >= from && idx <= last {
                from + (last - idx)
            } else {
                idx
            }
        };
        self.current = flip(self.current);
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let (a, b) = (flip(a), flip(b));
            self.loop_range = Some(if a <= b { (a, b) } else { (b, a) });
        }
        self.status = format!("Reversed frames {}–{}.", from + 1, from + count);
    }

    /// After frames [from, from+count) are deleted, keep `current`
    /// and the loop range attached to surviving frames where they
    /// can. A current frame inside the deleted block lands on the
    /// next survivor (or the last survivor when the tail was cut);
    /// a loop range that only overlapped the block shrinks to its
    /// surviving part, and one wholly inside it is cleared.
    fn remap_after_frame_delete(&mut self, from: usize, count: usize) {
        let end = from + count;
        let new_len = self.project.frames.len();
        if self.current >= from {
            self.current = if self.current < end {
                from.min(new_len - 1)
            } else {
                self.current - count
            };
        }
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let has_prefix = a < from;
            let has_suffix = b >= end;
            self.loop_range = match (has_prefix, has_suffix) {
                (false, false) => None,
                (true, true) => Some((a, b - count)),
                (true, false) => Some((a, b.min(from - 1))),
                (false, true) => {
                    let a = if a >= end { a - count } else { from };
                    Some((a, b - count))
                }
            };
        }
    }

    /// Delete the block of `count` frames starting at `from`.
    /// Poses, bends, durations and easings all live in `Frame`, so
    /// removal is structural. The count is clamped to the timeline
    /// end, like `reverse_range`; an empty or out-of-range block is
    /// refused, as is deleting every frame — a project always keeps
    /// at least one frame. Frame-indexed project data outside
    /// `Frame` (camera keys and FX clip start frames) is NOT
    /// remapped, matching `shift_frames`/`reverse_range`.
    pub fn delete_range(&mut self, from: usize, count: usize) -> bool {
        let n = self.project.frames.len();
        if count == 0 || from >= n {
            return false;
        }
        let count = count.min(n - from);
        if count == n {
            return false;
        }
        self.snapshot();
        self.project.frames.drain(from..from + count);
        self.remap_after_frame_delete(from, count);
        self.status = format!(
            "Deleted {count} frame(s) (frames {}–{}).",
            from + 1,
            from + count
        );
        true
    }

    /// Copy the block of `count` frames starting at `from` into the
    /// range clipboard as deep clones, ready for `paste_range`. The
    /// count is clamped to the timeline end; an empty or
    /// out-of-range block is refused and leaves any previous range
    /// clipboard untouched. This does not use the single-frame
    /// clipboard and, like `copy_frame`, is not an undoable edit.
    pub fn copy_range(&mut self, from: usize, count: usize) -> bool {
        let n = self.project.frames.len();
        if count == 0 || from >= n {
            return false;
        }
        let count = count.min(n - from);
        self.frame_clipboard = self.project.frames[from..from + count].to_vec();
        self.status = format!(
            "Copied {count} frame(s) (frames {}–{}).",
            from + 1,
            from + count
        );
        true
    }

    /// After frames are inserted at `at`, keep `current` and the
    /// loop range pointing at the same frames they pointed at
    /// before: indices at or after the insertion point shift later.
    fn remap_after_frame_insert(&mut self, at: usize, count: usize) {
        if self.current >= at {
            self.current += count;
        }
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let a = if a >= at { a + count } else { a };
            let b = if b >= at { b + count } else { b };
            self.loop_range = Some((a, b));
        }
    }

    /// Insert the range clipboard's frames at index `at` (clamped
    /// to the timeline, so a large `at` appends), shifting later
    /// frames. Refused when the range clipboard is empty, or when a
    /// copied frame's figure count no longer matches the project
    /// (the same guard as `paste_frame`). Frame-indexed project
    /// data outside `Frame` (camera keys and FX clip start frames)
    /// is NOT remapped, matching `shift_frames`/`reverse_range`.
    pub fn paste_range(&mut self, at: usize) -> bool {
        if self.frame_clipboard.is_empty() {
            return false;
        }
        if self
            .frame_clipboard
            .iter()
            .any(|frame| frame.poses.len() != self.project.figures.len())
        {
            self.status = "Copied frames have a different figure count.".into();
            return false;
        }
        let at = at.min(self.project.frames.len());
        let block = self.frame_clipboard.clone();
        let count = block.len();
        self.snapshot();
        for (i, frame) in block.into_iter().enumerate() {
            self.project.frames.insert(at + i, frame);
        }
        self.remap_after_frame_insert(at, count);
        self.status = format!("Pasted {count} frame(s) at position {}.", at + 1);
        true
    }

    /// Motion-path editing primitive: move the ROOT of figure `fi`'s
    /// pose at frame `frame` by `delta`. The roots across frames
    /// are what the motion path draws, so this bends the path at
    /// exactly one point; every other frame, and this frame's
    /// angles, are untouched.
    pub fn nudge_path_point(&mut self, frame: usize, fi: usize, delta: V2) {
        let valid = self
            .project
            .frames
            .get(frame)
            .map(|f| fi < f.poses.len())
            .unwrap_or(false);
        if !valid {
            self.status = "No such frame or figure to nudge.".into();
            return;
        }
        self.snapshot();
        let root = &mut self.project.frames[frame].poses[fi].root;
        root[0] += delta[0];
        root[1] += delta[1];
        self.status = format!(
            "Nudged path point at frame {} by ({:.0}, {:.0}).",
            frame + 1,
            delta[0],
            delta[1]
        );
    }

    /// One smoothing pass over figure `fi`'s motion path: each
    /// interior frame's root becomes the midpoint of its two
    /// neighbours' roots, computed from the pre-pass values (a
    /// single spike collapses to zero). The first and last frames
    /// keep their roots, and pose angles are never touched.
    pub fn smooth_path_roots(&mut self, fi: usize) {
        let n = self.project.frames.len();
        if n < 3 || fi >= self.project.figures.len() {
            self.status = "Nothing to smooth.".into();
            return;
        }
        // Compute the whole pass from the original roots first, so
        // a smoothed point never feeds its own neighbours.
        let roots: Vec<Option<V2>> = self
            .project
            .frames
            .iter()
            .map(|f| f.poses.get(fi).map(|p| p.root))
            .collect();
        let mut updates: Vec<(usize, V2)> = Vec::new();
        for f in 1..n - 1 {
            if let (Some(prev), Some(cur), Some(next)) = (roots[f - 1], roots[f], roots[f + 1]) {
                let mid = [(prev[0] + next[0]) / 2.0, (prev[1] + next[1]) / 2.0];
                if mid != cur {
                    updates.push((f, mid));
                }
            }
        }
        if updates.is_empty() {
            self.status = "Motion path already smooth.".into();
            return;
        }
        self.snapshot();
        let changed = updates.len();
        for (f, root) in updates {
            self.project.frames[f].poses[fi].root = root;
        }
        self.status = format!(
            "Smoothed motion path of figure {} ({changed} point(s)).",
            fi + 1
        );
    }

    pub fn positions_f(&self, frame_f: f32, fi: usize) -> Vec<V2> {
        if self.project.channels.is_empty() {
            let poses = self.project.poses_at(frame_f);
            let fig = &self.project.figures[fi];
            evaluate(&fig.def, &poses[fi])
        } else {
            // Wave 5: graph channels override the sampled pose/def.
            let r = crate::graph::apply_channels(&self.project, &self.project.channels, frame_f);
            evaluate(&r.defs[fi], &r.poses[fi])
        }
    }

    // ---- Wave 5 integration ops -------------------------------------
    /// Reorder the selected figure with the full layers engine:
    /// poses, graph channels, FX clips, particle refs and camera
    /// follow all travel with the figure.
    pub fn move_figure_full(&mut self, dir: i32) {
        let fi = self.selected_figure;
        let n = self.project.figures.len();
        if n == 0 {
            return;
        }
        let to = (fi as i32 + dir).clamp(0, n as i32 - 1) as usize;
        if to == fi {
            return;
        }
        self.snapshot();
        if crate::layers::move_figure(&mut self.project, fi, dir) {
            self.selected_figure = crate::layers::remapped_index(self.selected_figure, fi, to);
            if let Some(solo) = self.solo {
                self.solo = Some(crate::layers::remapped_index(solo, fi, to));
            }
            self.status = format!("Figure moved to position {}.", to + 1);
        }
    }



    /// Rotate the camera at the current frame (wave 6): upserts a
    /// camera key carrying the existing pan/zoom plus the new angle.
    pub fn rotate_camera(&mut self, delta_deg: f32) {
        self.snapshot();
        let frame = self.current;
        let (x, y, zoom) = self.project.camera_at(frame as f32);
        let angle = crate::model::norm_deg(self.project.camera_angle_at(frame as f32) + delta_deg);
        self.project.set_camera_key(frame, x, y, zoom);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = angle;
        }
        self.status = format!("Camera rotation {angle:.0}° at frame {}.", frame + 1);
    }

    /// Cycle the selected node's bend on the current frame
    /// (wave 7): 0 → +0.35 → +0.7 → −0.35 → −0.7 → 0 radians.
    /// Off-cycle values (e.g. imported bends) snap to the nearest
    /// step, then advance. Bends live in `Frame.bends[figure]
    /// [node]`; the origin has no segment and cannot bend.
    pub fn cycle_bend(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then cycle its bend.".into();
            return;
        };
        if ni == 0 {
            self.status = "The origin has no segment to bend.".into();
            return;
        }
        let fi = self.selected_figure;
        if fi >= self.project.figures.len() || self.current >= self.project.frames.len() {
            return;
        }
        const STEPS: [f32; 5] = [0.0, 0.35, 0.7, -0.35, -0.7];
        let cur = self.project.frames[self.current]
            .bends
            .get(fi)
            .and_then(|b| b.get(ni))
            .copied()
            .unwrap_or(0.0);
        let nearest = STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (cur - **a)
                    .abs()
                    .partial_cmp(&(cur - **b).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let next = STEPS[(nearest + 1) % STEPS.len()];
        self.snapshot();
        let nodes = self.project.figures[fi].def.nodes.len();
        let frame = &mut self.project.frames[self.current];
        if frame.bends.len() <= fi {
            frame.bends.resize(fi + 1, Vec::new());
        }
        if frame.bends[fi].len() < nodes {
            frame.bends[fi].resize(nodes, 0.0);
        }
        frame.bends[fi][ni] = next;
        self.status = format!(
            "Bend: figure {} node {} \u{2192} {:.0}\u{00b0}.",
            fi + 1,
            ni,
            next.to_degrees()
        );
    }

    pub fn toggle_figure_lock(&mut self, fi: usize) {
        match crate::layers::toggle_lock(&mut self.project, fi) {
            Some(locked) => {
                self.status = format!("Figure {} {}.", fi + 1, if locked { "locked" } else { "unlocked" })
            }
            None => self.status = "No such figure.".into(),
        }
    }

    pub fn cycle_figure_opacity(&mut self, fi: usize) {
        match crate::layers::cycle_opacity(&mut self.project, fi) {
            Some(o) => self.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0),
            None => self.status = "No such figure.".into(),
        }
    }

    /// Step a figure's opacity by `delta` (the sidebar's −/+ steppers),
    /// via the layers engine's clamped setter — so the bounds and NaN
    /// handling match every other opacity write. The target is rounded
    /// to two decimals so repeated steps land on clean percentages.
    /// Like `cycle_figure_opacity`, this does not snapshot.
    pub fn step_figure_opacity(&mut self, fi: usize, delta: f32) {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "No such figure.".into();
            return;
        };
        let next = ((fig.opacity + delta) * 100.0).round() / 100.0;
        if crate::layers::set_opacity(&mut self.project, fi, next) {
            let o = self.project.figures[fi].opacity;
            self.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0);
        } else {
            self.status = "No such figure.".into();
        }
    }

    pub fn toggle_solo(&mut self, fi: usize) {
        if self.solo == Some(fi) {
            self.solo = None;
            self.status = "Solo off.".into();
        } else if fi < self.project.figures.len() {
            self.solo = Some(fi);
            self.status = format!("Solo: figure {}.", fi + 1);
        }
    }

    /// Key the selected node's angle on a graph channel at the current
    /// frame, using its current keyframed value.
    pub fn key_graph_angle(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then Key graph.".into();
            return;
        };
        let fi = self.selected_figure;
        let frame = self.current as f32;
        let value = self
            .project
            .frames
            .get(self.current)
            .and_then(|f| f.poses.get(fi))
            .and_then(|p| p.angles.get(ni).copied())
            .unwrap_or(0.0);
        let ch = crate::graph::channel_mut(
            &mut self.project.channels,
            fi,
            crate::graph::ChannelTarget::Angle(ni),
        );
        ch.add_key(frame, value);
        self.status = format!("Graph key: figure {} joint {} = {value:.0}° @ frame {}.", fi + 1, ni, self.current + 1);
    }

    /// Remove all graph channels (the Graph panel edits them in a
    /// later pass; clearing must always be possible).
    pub fn clear_graph(&mut self) {
        self.project.channels.clear();
        self.status = "Graph channels cleared.".into();
    }

    /// Wave 10: smooth every graph channel's key tangents in one
    /// undo step. `graph::smooth_channel` fits automatic
    /// Catmull-Rom-style tangents through each channel's keys so
    /// Hermite segments flow instead of cornering at flat tangents;
    /// it derives from the channel's own target whether the values
    /// are angles (smoothed the short way around the circle) or
    /// plain scalars.
    pub fn smooth_graph_tangents(&mut self) {
        if self.project.channels.is_empty() {
            self.status = "No graph channels to smooth.".into();
            return;
        }
        let n = self.project.channels.len();
        self.snapshot();
        for ch in &mut self.project.channels {
            crate::graph::smooth_channel(ch);
        }
        self.status = format!("Smoothed tangents on {n} graph channel(s).");
    }

    /// Wave 12: apply a named tangent preset
    /// (`crate::graph::TangentPreset`) to every graph channel
    /// belonging to the selected figure, as one undo step. Angle
    /// channels are shaped with shortest-path (wrapping) deltas
    /// and every other target with plain deltas — the same
    /// `angular` derivation `graph::smooth_channel` uses — and
    /// each channel's keys are sorted first, as that wrapper
    /// does. Channels belonging to other figures are untouched;
    /// a selected figure with no channels is a no-op (no
    /// snapshot).
    pub fn apply_graph_ease_preset(&mut self, preset: crate::graph::TangentPreset) {
        let fi = self.selected_figure;
        let n = self
            .project
            .channels
            .iter()
            .filter(|ch| ch.figure == fi)
            .count();
        if n == 0 {
            self.status = format!("No graph channels for figure {}.", fi + 1);
            return;
        }
        self.snapshot();
        for ch in &mut self.project.channels {
            if ch.figure != fi {
                continue;
            }
            ch.sort_keys();
            let angular = matches!(ch.target, crate::graph::ChannelTarget::Angle(_));
            crate::graph::apply_tangent_preset(&mut ch.keys, preset, angular);
        }
        self.status = format!(
            "Applied {preset:?} preset to {n} graph channel(s) for figure {}.",
            fi + 1
        );
    }

    /// Wave 15: set the interpolation mode (`crate::graph::Interp`)
    /// of one graph key, as one undo step.
    ///
    /// Which key: `EditorCore` has no graph key selection of its
    /// own — the graph panel's channel selector and its key
    /// selection (`graph_sel`) live in the view, and the core-side
    /// graph ops scope by figure and joint instead (the tangent
    /// presets apply to every channel of the selected figure).
    /// So this op targets the same channel `key_graph_angle`
    /// authors — the selected figure's `Angle(selected_node)`
    /// channel — and within it the key nearest the current frame
    /// (an exact hit wins; ties go to the earlier key). The mode
    /// governs the segment STARTING at that key (see
    /// `graph::Interp`). No selected joint, or a channel with no
    /// keys, is a no-op (no snapshot).
    pub fn set_graph_key_interp(&mut self, mode: crate::graph::Interp) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then set graph interp.".into();
            return;
        };
        let fi = self.selected_figure;
        let target = crate::graph::ChannelTarget::Angle(ni);
        let Some(ci) = self
            .project
            .channels
            .iter()
            .position(|ch| ch.figure == fi && ch.target == target)
        else {
            self.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
            return;
        };
        let now = self.current as f32;
        let mut best: Option<usize> = None;
        let mut best_dist = f32::INFINITY;
        for (ki, key) in self.project.channels[ci].keys.iter().enumerate() {
            let dist = (key.frame - now).abs();
            if dist < best_dist {
                best_dist = dist;
                best = Some(ki);
            }
        }
        let Some(ki) = best else {
            self.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
            return;
        };
        self.snapshot();
        let key = &mut self.project.channels[ci].keys[ki];
        key.interp = mode;
        let frame = key.frame;
        self.status = format!(
            "Interp {mode:?} set on key at frame {}.",
            frame.round() as i64 + 1
        );
    }

    /// Wave 11: bake every graph channel into real frame poses
    /// for frames `from..=to` (the end clamped to the timeline),
    /// as one undo step.
    ///
    /// Each frame's poses are computed with
    /// `crate::graph::apply_channels` — the same evaluation the
    /// renderer uses — so the baked frames hold exactly what the
    /// channels produced, and the channels can then be edited or
    /// cleared without changing those frames. Every frame is
    /// evaluated BEFORE the snapshot and the writes, so a bake
    /// never leaves a half-baked range behind. Stated plainly:
    /// length channels are not baked — `apply_channels` reports
    /// them through the returned figure defs, and frame poses
    /// carry no lengths, so only root and angle channels land in
    /// the frames. The status counts every channel either way.
    pub fn bake_graph_channels(&mut self, from: usize, to: usize) {
        if self.project.channels.is_empty() {
            self.status = "No graph channels to bake.".into();
            return;
        }
        if self.project.frames.is_empty() || from > to || from >= self.project.frames.len() {
            self.status = "Graph bake failed: invalid frame range.".into();
            return;
        }
        let lo = from;
        let hi = to.min(self.project.frames.len() - 1);
        let n_channels = self.project.channels.len();
        let baked: Vec<Vec<crate::model::Pose>> = (lo..=hi)
            .map(|f| {
                crate::graph::apply_channels(&self.project, &self.project.channels, f as f32)
                    .poses
            })
            .collect();
        self.snapshot();
        for (poses, f) in baked.into_iter().zip(lo..=hi) {
            self.project.frames[f].poses = poses;
        }
        self.status = format!(
            "Baked {n_channels} graph channel(s) into frames {}–{} ({} frame(s)).",
            lo + 1,
            hi + 1,
            hi - lo + 1
        );
    }

    /// Wave 10: mirror the selected figure's pose on the current
    /// frame, in place, as one undo step.
    ///
    /// Pose angles are RELATIVE joint angles in degrees, so
    /// negating every angle reflects the pose about the vertical
    /// axis through the root: each segment leaves its parent at
    /// the mirrored heading, and the root position is unchanged.
    /// Approximation, stated honestly: this is exact for the angle
    /// data itself and for bilaterally symmetric rigs (the
    /// built-in presets) — mirroring twice returns the original
    /// pose exactly. It does not touch anything outside `Pose`:
    /// per-node bends, segment lengths and graph channels keep
    /// their values, so a mirrored pose driven by a graph channel
    /// or carrying bends can differ slightly from a true
    /// geometric mirror of the rendered figure.
    pub fn mirror_selected_pose(&mut self) {
        let fi = self.selected_figure;
        if fi >= self.project.figures.len() {
            self.status = "No such figure.".into();
            return;
        }
        let has_pose = self
            .project
            .frames
            .get(self.current)
            .and_then(|f| f.poses.get(fi))
            .is_some();
        if !has_pose {
            self.status = format!(
                "Figure {} has no pose on frame {}.",
                fi + 1,
                self.current + 1
            );
            return;
        }
        self.snapshot();
        let pose = &mut self.project.frames[self.current].poses[fi];
        for a in &mut pose.angles {
            *a = -*a;
        }
        self.status = format!("Mirrored figure {} on frame {}.", fi + 1, self.current + 1);
    }

    /// Wave 11: copy the current frame's pose of figure
    /// `from_fig` onto figure `to_fig` (same frame), as one undo
    /// step. The pose is copied exactly — root and every angle —
    /// so the target figure adopts the source's posture while
    /// keeping its own definition, bends and lengths.
    pub fn copy_pose_between(&mut self, from_fig: usize, to_fig: usize) {
        if from_fig >= self.project.figures.len() || to_fig >= self.project.figures.len() {
            self.status = "Copy pose failed: figure index out of range.".into();
            return;
        }
        if from_fig == to_fig {
            self.status = "Copy pose failed: source and target are the same figure.".into();
            return;
        }
        let cur = self.current;
        let Some(pose) = self
            .project
            .frames
            .get(cur)
            .and_then(|f| f.poses.get(from_fig))
            .cloned()
        else {
            self.status = format!("Figure {} has no pose on frame {}.", from_fig + 1, cur + 1);
            return;
        };
        if self.project.frames[cur].poses.get(to_fig).is_none() {
            self.status = format!("Figure {} has no pose on frame {}.", to_fig + 1, cur + 1);
            return;
        }
        self.snapshot();
        self.project.frames[cur].poses[to_fig] = pose;
        self.status = format!(
            "Copied figure {}'s pose to figure {} on frame {}.",
            from_fig + 1,
            to_fig + 1,
            cur + 1
        );
    }

    pub fn open_via_dialog(&mut self) {
        if crate::dialogs::available_tool().is_none() {
            self.status =
                "No dialog tool (zenity/kdialog) on this system — use the project list instead."
                    .into();
            return;
        }
        let filters = vec![crate::dialogs::FileFilter::new(
            "StykAnimate project",
            &["*.json"],
        )];
        let Some(path) = crate::dialogs::open_file("Open project", &filters) else {
            self.status = "Open cancelled.".into();
            return;
        };
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|txt| serde_json::from_str::<Project>(&txt).ok())
        {
            Some(p) => {
                self.project = p;
                self.current = 0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.solo = None;
                self.status = format!("Opened {}", path.display());
            }
            None => self.status = format!("Could not open {} as a project.", path.display()),
        }
    }

    pub fn save_via_dialog(&mut self) {
        if crate::dialogs::available_tool().is_none() {
            self.status =
                "No dialog tool (zenity/kdialog) on this system — use Save project instead."
                    .into();
            return;
        }
        let filters = vec![crate::dialogs::FileFilter::new(
            "StykAnimate project",
            &["*.json"],
        )];
        let Some(path) = crate::dialogs::save_file("Save project", &filters, "project.styk.json")
        else {
            self.status = "Save cancelled.".into();
            return;
        };
        match serde_json::to_string_pretty(&self.project)
            .map_err(std::io::Error::other)
            .and_then(|txt| std::fs::write(&path, txt))
        {
            Ok(()) => self.status = format!("Saved to {}", path.display()),
            Err(e) => self.status = format!("Save failed: {e}"),
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

    /// Wave 9: autosave the recovery snapshot after this many
    /// snapshot-counted edits (see `maybe_autosave`).
    pub const AUTOSAVE_EVERY: u64 = 25;

    /// Wave 9: the pure autosave decision, factored out of
    /// `maybe_autosave` so the threshold logic is unit tested
    /// without touching the filesystem.
    fn autosave_due(edit_count: u64, last_autosave_count: u64) -> bool {
        edit_count.saturating_sub(last_autosave_count) >= Self::AUTOSAVE_EVERY
    }

    /// Wave 9: snapshot-counted edits not yet covered by an
    /// autosave (0 right after a successful one).
    pub fn edits_since_autosave(&self) -> u64 {
        self.edit_count.saturating_sub(self.last_autosave_count)
    }

    /// Wave 9: if at least `AUTOSAVE_EVERY` edits have
    /// accumulated since the last autosave, write the recovery
    /// snapshot (`./projects/.recovery.json`, via shell.rs — the
    /// same file the Home panel writes and the Recover button
    /// reads). Silent on success: autosave must never spam the
    /// status line. On failure the status reports it and the
    /// last-autosave counter is left alone, so the next call
    /// retries instead of waiting for another AUTOSAVE_EVERY
    /// edits. Nothing calls this yet — the app layer decides
    /// when to check (e.g. after ops); this is only the
    /// mechanism.
    pub fn maybe_autosave(&mut self) {
        if !Self::autosave_due(self.edit_count, self.last_autosave_count) {
            return;
        }
        match crate::shell::write_recovery(&self.project) {
            Ok(_) => self.last_autosave_count = self.edit_count,
            Err(e) => self.status = format!("Autosave failed: {e}"),
        }
    }

    // ---- Figure / layer management ----

    pub fn delete_figure(&mut self) {
        if self.project.figures.len() <= 1 {
            self.status = "Keep at least one figure.".into();
            return;
        }
        self.snapshot();
        let fi = self.selected_figure.min(self.project.figures.len() - 1);
        self.project.figures.remove(fi);
        for frame in &mut self.project.frames {
            frame.poses.remove(fi);
        }
        // Every figure-indexed reference follows the removal
        // (Phase F figure_ops census: particles, FX clips,
        // graph channels, camera follow) — refs to the removed
        // figure drop, refs above it shift down.
        crate::figure_ops::remap_figures_after_remove(&mut self.project, fi);
        if self.solo == Some(fi) {
            self.solo = None;
        } else if let Some(s) = self.solo {
            if s > fi {
                self.solo = Some(s - 1);
            }
        }
        self.selected_figure = fi.min(self.project.figures.len() - 1);
        self.selected_node = None;
        self.status = "Figure deleted.".into();
    }

    /// Move the selected figure later (drawn on top) or earlier.
    pub fn move_figure(&mut self, dir: i32) {
        let fi = self.selected_figure;
        let target = fi as i32 + dir;
        if target < 0 || target as usize >= self.project.figures.len() {
            return;
        }
        self.snapshot();
        let target = target as usize;
        self.project.figures.swap(fi, target);
        for frame in &mut self.project.frames {
            frame.poses.swap(fi, target);
            // Bend rows are per-figure like pose rows;
            // swap them where both rows exist (the table
            // is sparse — a missing row reads as 0.0).
            if fi < frame.bends.len() && target < frame.bends.len() {
                frame.bends.swap(fi, target);
            }
        }
        // Figure-indexed references ride the swap
        // (Phase G): particle anchors, FX clips, graph
        // channels and the camera-follow target.
        crate::figure_ops::remap_figures_after_move(&mut self.project, fi, target);
        if let Some(s) = self.solo {
            self.solo = Some(crate::figure_ops::remap_index_after_move(s, fi, target));
        }
        self.selected_figure = target;
        self.status = "Figure order changed.".into();
    }

    pub fn toggle_figure_hidden(&mut self) {
        let fi = self.selected_figure;
        let fig = &mut self.project.figures[fi];
        fig.hidden = !fig.hidden;
        self.status = format!("{} {}.", fig.name, if fig.hidden { "hidden" } else { "shown" });
    }

    pub fn select_figure(&mut self, fi: usize) {
        if fi < self.project.figures.len() {
            self.selected_figure = fi;
            self.selected_node = None;
        }
    }

    // ---- Frame clipboard, timing, project settings ----

    pub fn copy_frame(&mut self) {
        self.clipboard = Some(self.project.frames[self.current].clone());
        self.status = format!("Frame {} copied.", self.current + 1);
    }

    pub fn paste_frame(&mut self) {
        let Some(frame) = self.clipboard.clone() else {
            self.status = "Nothing copied yet.".into();
            return;
        };
        if frame.poses.len() != self.project.figures.len() {
            self.status = "Copied frame has a different figure count.".into();
            return;
        }
        self.snapshot();
        self.project.frames.insert(self.current + 1, frame);
        self.current += 1;
        self.status = format!("Frame pasted at {}.", self.current + 1);
    }

    pub fn adjust_frame_duration(&mut self, factor: f32) {
        let frame = &mut self.project.frames[self.current];
        frame.duration = (frame.duration * factor).clamp(0.25, 8.0);
        self.status = format!("Frame {} duration {:.2}x.", self.current + 1, frame.duration);
    }

    pub fn adjust_fps(&mut self, delta: f32) {
        self.project.fps = (self.project.fps + delta).clamp(1.0, 60.0);
        self.status = format!("{} fps.", self.project.fps as i32);
    }

    pub fn cycle_easing(&mut self) {
        self.project.easing = self.project.easing.next();
        self.status = format!("Easing: {}.", self.project.easing.label());
    }

    /// Set the project easing directly (the easing picker's
    /// preset buttons). Like `cycle_easing` it does not
    /// snapshot — project-settings tweaks are not undoable
    /// edits in this core.
    pub fn set_project_easing(&mut self, easing: crate::model::Easing) {
        self.project.easing = easing;
        self.status = format!("Easing: {}.", self.project.easing.label());
    }

    pub fn toggle_tween_playback(&mut self) {
        self.project.tween_playback = !self.project.tween_playback;
        self.status = format!(
            "Tween playback {}.",
            if self.project.tween_playback { "on" } else { "off" }
        );
    }

    pub fn toggle_camera_follow(&mut self) {
        self.project.camera_follow = if self.project.camera_follow == Some(self.selected_figure) {
            None
        } else {
            Some(self.selected_figure)
        };
        self.status = match self.project.camera_follow {
            Some(fi) => format!("Camera follows {}.", self.project.figures[fi].name),
            None => "Camera follow off.".into(),
        };
    }

    // ---- Particles ----

    const PARTICLE_KINDS: [crate::model::ParticleKind; 3] = [
        crate::model::ParticleKind::Burst,
        crate::model::ParticleKind::Sparkle,
        crate::model::ParticleKind::Smoke,
    ];

    pub fn add_particle(&mut self, kind_idx: usize) {
        self.snapshot();
        let fi = self.selected_figure;
        let (figure, node, x, y) = match self.selected_node {
            Some(ni) => {
                let pos = self.positions(self.current, fi);
                (Some(fi), Some(ni), pos[ni][0], pos[ni][1])
            }
            None => {
                let pos = self.positions(self.current, fi);
                (Some(fi), Some(0), pos[0][0], pos[0][1])
            }
        };
        let color = self.project.figures[fi].color;
        let seed = (self.project.particles.len() as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(12345);
        self.project.particles.push(crate::model::ParticleEvent {
            frame: self.current,
            figure,
            node,
            x,
            y,
            kind: Self::PARTICLE_KINDS[kind_idx % Self::PARTICLE_KINDS.len()],
            color,
            seed,
        });
        self.status = "Particle effect added on this frame.".into();
    }

    pub fn clear_particles(&mut self) {
        self.snapshot();
        self.project.particles.clear();
        self.project.fx_clips.clear();
        self.status = "Particle effects cleared.".into();
    }

    // ---- Wave-4 integrations: nodes animation, styles, media, mixdown ----

    pub fn import_nodes_anim_file(&mut self) {
        let Some(path) = self.first_file_with("import", &["nodemc", "stknds"]) else {
            self.status = "Put a .nodemc or .stknds file in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match crate::nodes_anim::import_nodes_animation(&bytes) {
                Ok(project) => {
                    self.snapshot();
                    self.project = project;
                    self.current = 0;
                    self.playhead = 0.0;
                    self.selected_figure = 0;
                    self.selected_node = None;
                    self.status = format!("Imported {}.", path.display());
                }
                Err(e) => self.status = format!("Stick Nodes animation import failed: {e}"),
            },
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    fn selected_node_mut(&mut self) -> Option<&mut crate::model::NodeDef> {
        let ni = self.selected_node?;
        self.project.figures.get_mut(self.selected_figure)?.def.nodes.get_mut(ni)
    }

    pub fn cycle_node_cap(&mut self) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.cap = crate::style::cycle_cap(node.style.cap);
                self.status = format!("Segment cap: {:?}", node.style.cap);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    pub fn cycle_node_taper(&mut self) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.taper = match node.style.taper {
                    t if t > 0.9 => 0.6,
                    t if t > 0.5 => 0.3,
                    t if t > 0.1 => 0.0,
                    _ => 1.0,
                };
                self.status = format!("Segment {}", crate::style::taper_label(node.style.taper));
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    pub fn cycle_node_gradient(&mut self) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.color2 = match node.style.color2 {
                    None => Some(0xFFFFFF),
                    Some(0xFFFFFF) => Some(0x222222),
                    _ => None,
                };
                self.status = match node.style.color2 {
                    Some(c) => format!("Gradient to #{c:06X}."),
                    None => "Gradient off (solid colour).".into(),
                };
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's cap directly (the style
    /// panel's segmented control) instead of cycling it.
    /// Snapshots and reports exactly like `cycle_node_cap`.
    pub fn set_node_cap(&mut self, cap: crate::model::CapKind) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.cap = cap;
                self.status = format!("Segment cap: {:?}", node.style.cap);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's gradient end colour
    /// directly, or clear it with `None` (the style panel's
    /// swatch row). Snapshots and reports exactly like
    /// `cycle_node_gradient`.
    pub fn set_node_color2(&mut self, color2: Option<u32>) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.color2 = color2;
                self.status = match node.style.color2 {
                    Some(c) => format!("Gradient to #{c:06X}."),
                    None => "Gradient off (solid colour).".into(),
                };
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's opacity directly (the
    /// style panel's stepper), clamped to 0.05..=1.0 so a
    /// segment can never be made fully invisible by accident.
    /// Snapshots and reports like the other style setters.
    pub fn set_node_opacity(&mut self, opacity: f32) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.style.opacity = opacity.clamp(0.05, 1.0);
                self.status = format!("Segment opacity {:.0}%.", node.style.opacity * 100.0);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    pub fn attach_image_to_node(&mut self) {
        let Some(node_idx) = self.selected_node else {
            self.status = "Select a joint first.".into();
            return;
        };
        let Some(path) = self.first_file_with("import", &["png"]) else {
            self.status = "Put a PNG in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                self.snapshot();
                let asset = self.project.assets.len();
                self.project.assets.push(crate::model::ImageAsset {
                    name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    png: bytes,
                });
                if let Some(node) = self
                    .project
                    .figures
                    .get_mut(self.selected_figure)
                    .and_then(|f| f.def.nodes.get_mut(node_idx))
                {
                    node.media = Some(crate::model::SegmentMedia::Image { asset });
                    self.status = format!("Sprite attached (asset {}).", asset + 1);
                }
            }
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    pub fn clear_node_media(&mut self) {
        self.snapshot();
        match self.selected_node_mut() {
            Some(node) => {
                node.media = None;
                self.status = "Segment media cleared.".into();
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    pub fn mixdown_audio(&mut self) {
        if self.project.audio.is_empty() {
            self.status = "No audio clips to mix.".into();
            return;
        }
        let fps = self.project.fps.max(1.0);
        let clips: Vec<crate::audio_edit::MixClip> = self
            .project
            .audio
            .iter()
            .map(|c| crate::audio_edit::MixClip {
                path: c.path.clone(),
                start_secs: c.start_frame as f32 / fps,
                gain: c.gain,
                fade_in: 0.0,
                fade_out: 0.0,
            })
            .collect();
        match crate::audio_edit::mix(&clips) {
            Ok(wav) => {
                let bytes = crate::audio_edit::write_wav(&wav);
                let dir = PathBuf::from("out");
                let _ = std::fs::create_dir_all(&dir);
                match std::fs::write(dir.join("mixdown.wav"), bytes) {
                    Ok(()) => self.status = "Mixed audio to out/mixdown.wav.".into(),
                    Err(e) => self.status = format!("Mixdown write failed: {e}"),
                }
            }
            Err(e) => self.status = format!("Mixdown failed: {e}"),
        }
    }

    // ---- Wave-10 integrations: formats, language, easing, VFX clips ----

    pub fn cycle_lang(&mut self) {
        // Cycle through every language the i18n tables know about, so
        // this keeps working as the language list grows.
        let all = crate::i18n::Lang::all();
        let idx = all.iter().position(|l| *l == self.lang).unwrap_or(0);
        self.lang = all[(idx + 1) % all.len()];
        self.status = format!("Language: {}", self.lang.label());
    }

    pub fn export_v5_file(&mut self) {
        let bytes = crate::pivot::export_piv_v5(&self.project);
        let dir = PathBuf::from("out");
        let _ = std::fs::create_dir_all(&dir);
        match std::fs::write(dir.join("animation-v5.piv"), &bytes) {
            Ok(()) => self.status = "Exported out/animation-v5.piv (Pivot v5).".into(),
            Err(e) => self.status = format!("v5 export failed: {e}"),
        }
    }

    pub fn export_apng_file(&mut self) {
        match crate::export::export_apng(&self.project, std::path::Path::new("out/animation.apng")) {
            Ok(_) => self.status = "Exported out/animation.apng.".into(),
            Err(e) => self.status = format!("APNG export failed: {e}"),
        }
    }

    pub fn export_jpeg_dir(&mut self) {
        match crate::export::export_jpeg_sequence(&self.project, std::path::Path::new("out/jpeg")) {
            Ok(_) => self.status = "Exported JPEG frames to out/jpeg/.".into(),
            Err(e) => self.status = format!("JPEG export failed: {e}"),
        }
    }

    pub fn import_nodes_file(&mut self) {
        let Some(path) = self.first_file_with("import", &["nodes"]) else {
            self.status = "Put a .nodes file in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match crate::nodes::import_nodes(&bytes) {
                Ok(project) => {
                    self.snapshot();
                    self.project = project;
                    self.current = 0;
                    self.selected_figure = 0;
                    self.selected_node = None;
                    self.status = format!("Imported {} as a figure.", path.display());
                }
                Err(e) => self.status = format!("Stick Nodes import failed: {e}"),
            },
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    pub fn cycle_frame_easing(&mut self) {
        use crate::model::Easing;
        self.snapshot();
        let cur = self.current.min(self.project.frames.len().saturating_sub(1));
        if let Some(frame) = self.project.frames.get_mut(cur) {
            frame.easing = match frame.easing {
                None => Some(Easing::Linear),
                Some(Easing::Linear) => Some(Easing::Smooth),
                Some(Easing::Smooth) => Some(Easing::EaseIn),
                Some(Easing::EaseIn) => Some(Easing::EaseOut),
                Some(Easing::EaseOut) => None,
            };
            self.status = format!(
                "Frame {} easing: {}",
                cur + 1,
                frame.easing.map(|e| e.label()).unwrap_or("project default")
            );
        }
    }

    /// Set the CURRENT frame's easing directly (the easing
    /// picker's Apply / None buttons), with the same snapshot +
    /// status behaviour as `cycle_frame_easing`.
    pub fn set_frame_easing(&mut self, easing: Option<crate::model::Easing>) {
        self.snapshot();
        let cur = self.current.min(self.project.frames.len().saturating_sub(1));
        if let Some(frame) = self.project.frames.get_mut(cur) {
            frame.easing = easing;
            self.status = format!(
                "Frame {} easing: {}",
                cur + 1,
                frame.easing.map(|e| e.label()).unwrap_or("project default")
            );
        }
    }

    pub fn frame_easing_label(&self) -> String {
        match self.project.frames.get(self.current).and_then(|f| f.easing) {
            Some(e) => format!("F-ease: {}", e.label()),
            None => "F-ease: project".into(),
        }
    }

    /// Add a timed VFX clip of `kind` anchored at the selected node,
    /// starting at the current frame.
    pub fn add_fx_clip(&mut self, kind: crate::vfx::ClipKind) {
        use crate::vfx::{ClipKind, ParticleClip};
        let Some(node) = self.selected_node else {
            self.status = "Select a joint first, then add the FX clip.".into();
            return;
        };
        self.snapshot();
        let color = self
            .project
            .figures
            .get(self.selected_figure)
            .map(|f| f.color)
            .unwrap_or(0xFFFFFF);
        self.project.fx_clips.push(crate::model::FxClipEntry {
            clip: ParticleClip {
                start: self.current as f32,
                duration: 16.0,
                kind,
                color,
                seed: 11 + self.project.fx_clips.len() as u32 * 7,
                angle: 0.0,
                length: 130.0,
                color2: None,
            },
            figure: self.selected_figure,
            node,
        });
        let _ = ClipKind::Burst;
        self.status = format!("FX clip ({kind:?}) added at frame {}.", self.current + 1);
    }

    // ---- FX presets (wave 13) ----

    /// Wave 13: save FX clip `index` as a preset in
    /// `./fx-presets/`, auto-named `<kind>-<index+1>` (for
    /// example `sparkle-2`). Saving a preset does not change
    /// the project, so no snapshot is taken. A missing index or
    /// a failed write sets an error status.
    pub fn fx_save_preset(&mut self, index: usize) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX preset save failed: no FX clip {}.", index + 1);
            return;
        };
        let name = format!(
            "{}-{}",
            crate::fx_edit::kind_name(entry.clip.kind),
            index + 1
        );
        match crate::fx_edit::save_clip_preset(&entry.clip, &name) {
            Ok(path) => {
                self.status = format!("FX preset '{name}' saved to {}.", path.display());
            }
            Err(e) => self.status = format!("FX preset save failed: {e}"),
        }
    }

    /// Wave 13: add the preset `name` from `./fx-presets/` as a
    /// new FX clip on the selected figure (node 0), starting at
    /// the current frame — the placement shape of `add_fx_clip`,
    /// as one undo step. A preset that cannot be loaded sets an
    /// error status and changes nothing.
    pub fn fx_apply_preset(&mut self, name: &str) {
        let mut clip = match crate::fx_edit::load_clip_preset(name) {
            Ok(clip) => clip,
            Err(e) => {
                self.status = format!("FX preset apply failed: {e}");
                return;
            }
        };
        clip.start = self.current as f32;
        self.snapshot();
        self.project.fx_clips.push(crate::model::FxClipEntry {
            clip,
            figure: self.selected_figure,
            node: 0,
        });
        self.status = format!("FX preset '{name}' added at frame {}.", self.current + 1);
    }

    /// Wave 13: the slugs of the FX presets available in
    /// `./fx-presets/` (see `fx_edit::list_clip_presets`).
    pub fn fx_preset_names(&self) -> Vec<String> {
        crate::fx_edit::list_clip_presets()
    }


    // ---- Library ----

    pub fn cycle_library(&mut self) {
        if !self.library.is_empty() {
            self.library_index = (self.library_index + 1) % self.library.len();
            self.status = format!("Library: {}", self.library[self.library_index].0);
        }
    }

    pub fn insert_library_figure(&mut self) {
        if self.library.is_empty() {
            return;
        }
        self.snapshot();
        let (name, def) = self.library[self.library_index].clone();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [300.0 + 80.0 * idx as f32, 450.0];
        self.project.add_figure_with(def, &name, color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = format!("Inserted {name} from the library.");
    }

    pub fn save_figure_to_library(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        let file = dir.join(format!("{}.json", fig.name.replace(' ', "-").to_lowercase()));
        let result = std::fs::create_dir_all(&dir)
            .and_then(|_| serde_json::to_string_pretty(&fig.def).map_err(std::io::Error::other))
            .and_then(|text| std::fs::write(&file, text));
        match result {
            Ok(()) => {
                self.library = Self::load_library();
                self.status = format!("Saved {} to the library.", fig.name);
            }
            Err(e) => self.status = format!("Library save failed: {e}"),
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

    // ---- Pivot formats ----

    fn first_file_with(&self, dir: &str, exts: &[&str]) -> Option<PathBuf> {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(dir);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.extension()
                            .map(|x| exts.iter().any(|e| x.eq_ignore_ascii_case(e)))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files.into_iter().next()
    }

    pub fn import_piv(&mut self) {
        let Some(path) = self.first_file_with("import", &["piv"]) else {
            self.status = "Put a .piv file in ./import/ first.".into();
            return;
        };
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| crate::pivot::import_piv(&b)) {
            Ok(project) => {
                self.project = project;
                self.current = 0;
                self.playhead = 0.0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.undo.clear();
                self.redo.clear();
                self.status = format!("Imported {}", path.display());
            }
            Err(e) => self.status = format!("Import failed: {e}"),
        }
    }

    pub fn import_stk(&mut self) {
        let Some(path) = self.first_file_with("import", &["stk"]) else {
            self.status = "Put a .stk file in ./import/ first.".into();
            return;
        };
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| crate::pivot::import_stk(&b)) {
            Ok((def, name)) => {
                self.snapshot();
                let idx = self.project.figures.len();
                let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
                let label = if name.is_empty() { "Imported figure".to_string() } else { name };
                self.project.add_figure_with(def, &label, color, [320.0, 450.0]);
                self.selected_figure = idx;
                self.status = format!("Imported figure from {}", path.display());
            }
            Err(e) => self.status = format!("STK import failed: {e}"),
        }
    }

    pub fn export_piv(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation.piv");
        let bytes = crate::pivot::export_piv_v3(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v3) to {}", path.display()),
            Err(e) => self.status = format!("PIV export failed: {e}"),
        }
    }

    pub fn export_stk(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join(format!("{}.stk", fig.name.replace(' ', "-").to_lowercase()));
        let bytes = crate::pivot::export_stk(&fig.def, &fig.name);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported {}.stk", fig.name),
            Err(e) => self.status = format!("STK export failed: {e}"),
        }
    }

    pub fn export_piv2(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation-v2.piv");
        let bytes = crate::pivot::export_piv_v2(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v2) to {}", path.display()),
            Err(e) => self.status = format!("PIV v2 export failed: {e}"),
        }
    }

    pub fn export_piv4(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation-v4.piv");
        let bytes = crate::pivot::export_piv_v4(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v4) to {}", path.display()),
            Err(e) => self.status = format!("PIV v4 export failed: {e}"),
        }
    }

    pub fn export_nodes(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join(format!("{}.nodes", fig.name.replace(' ', "-").to_lowercase()));
        let bytes = crate::nodes::export_nodes_figure(&fig.def, &fig.name);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported {}.nodes", fig.name),
            Err(e) => self.status = format!("Nodes export failed: {e}"),
        }
    }

    pub fn export_stknds(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation.stknds");
        let bytes = crate::nodes_anim::export_nodes_animation(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Stick Nodes .stknds to {}", path.display()),
            Err(e) => self.status = format!("STKNDS export failed: {e}"),
        }
    }

    // ---- Drawing ----

    pub fn begin_draw(&mut self, p: V2) {
        if self.draw_tool == DrawTool::Eraser {
            // An eraser drag erases around the pointer instead of
            // recording a stroke; it continues in `draw_to`. One
            // snapshot covers the whole drag, like `lasso_erase`.
            self.snapshot();
            let frame = self.current;
            let radius = self.eraser_radius();
            self.erase_changed =
                crate::draw_ops::erase(&mut self.project, frame, p, radius);
            self.drawing = true;
            return;
        }
        let brush = self.active_brush();
        let color = self.project.figures[self.selected_figure].color;
        // Wave 13: a scatter brush (scatter > 0) records a spray
        // mark on the stroke. The seed is derived from the frame
        // and the stroke's own index in `project.strokes`, so the
        // scatter a renderer expands from the mark
        // (`paint::spray_dots`) is deterministic per stroke. The
        // recorded points stay the path the user drew; plain
        // brushes (scatter 0) record no mark.
        let spray = if brush.scatter > 0.0 {
            Some(crate::model::SprayMark {
                seed: 0x5EED_0000u64
                    + (self.current as u64) * 131
                    + self.project.strokes.len() as u64,
                radius: brush.scatter,
                opacity: brush.opacity,
            })
        } else {
            None
        };
        self.project.strokes.push(crate::model::Stroke {
            frame: self.current,
            points: vec![p],
            color,
            width: brush.width,
            widths: vec![brush.width],
            fill: None,
            spray,
        });
        self.drawing = true;
    }

    pub fn draw_to(&mut self, p: V2) {
        if !self.drawing {
            return;
        }
        if self.draw_tool == DrawTool::Eraser {
            let frame = self.current;
            let radius = self.eraser_radius();
            self.erase_changed +=
                crate::draw_ops::erase(&mut self.project, frame, p, radius);
            return;
        }
        if let Some(stroke) = self.project.strokes.last_mut() {
            let last = *stroke.points.last().unwrap();
            let d = dist(last, p);
            if d > 2.0 {
                stroke.points.push(p);
                // Pressure stroke: the recorded per-point width follows
                // pointer speed — slow movement lays down up to 1.8x the
                // base width, a fast flick tapers to 0.45x. Smoothed
                // against the previous width so the taper doesn't
                // jitter. `width` stays the base; `widths` stays
                // parallel to `points`.
                let base = stroke.width;
                while stroke.widths.len() < stroke.points.len() - 1 {
                    stroke.widths.push(base);
                }
                let prev = stroke.widths.last().copied().unwrap_or(base);
                let t = ((d - 2.0) / 28.0).clamp(0.0, 1.0);
                let target = base * (1.8 - t * (1.8 - 0.45));
                let w = (prev + (target - prev) * 0.5).clamp(base * 0.45, base * 1.8);
                stroke.widths.push(w);
            }
        }
    }

    pub fn end_draw(&mut self) {
        if !self.drawing {
            return;
        }
        self.drawing = false;
        if self.draw_tool == DrawTool::Eraser {
            self.status = format!("Eraser: {} stroke(s) changed.", self.erase_changed);
            self.erase_changed = 0;
            return;
        }
        // Finalise the stroke with the active brush's pipeline, in
        // this order: Chaikin smoothing first (it repositions the
        // points), then the nib profile (computed from the smoothed
        // path, replacing the recorded pressure widths). The
        // default Pen brush has smoothing 0 and a round nib, so
        // neither step runs for it and the recorded stroke is left
        // exactly as drawn.
        let brush = self.active_brush();
        if let Some(stroke) = self.project.strokes.last_mut() {
            if brush.smoothing > 0 {
                crate::draw_ops::smooth_stroke(stroke, brush.smoothing);
            }
            if brush.nib != crate::paint::Nib::Round {
                crate::draw_ops::apply_nib(stroke, brush.nib, brush.nib_angle);
            }
            let n = stroke.points.len();
            self.status = format!("Stroke drawn ({n} points).");
        }
    }

    /// Delete the stroke at `index` in `project.strokes`
    /// (the stroke inspector's per-stroke delete), with a
    /// snapshot so the delete is undoable. Out of range only
    /// reports — nothing is removed or snapshotted.
    pub fn delete_stroke_at(&mut self, index: usize) {
        if index >= self.project.strokes.len() {
            self.status = format!("No stroke {} to delete.", index + 1);
            return;
        }
        self.snapshot();
        self.project.strokes.remove(index);
        self.status = format!("Stroke {} deleted.", index + 1);
    }

    pub fn clear_frame_drawings(&mut self) {
        let before = self.project.strokes.len();
        self.project.strokes.retain(|s| s.frame != self.current);
        self.status = format!("Cleared {} stroke(s) on this frame.", before - self.project.strokes.len());
    }

    /// Fill the most recent stroke on this frame: the stroke's outline
    /// is treated as a closed shape, its `fill` is set to its own
    /// colour, and a serpentine scanline fill (see `paint`) is
    /// appended as a separate stroke on the same frame.
    pub fn fill_current_stroke(&mut self) {
        let frame = self.current;
        let Some(src) = self
            .project
            .strokes
            .iter()
            .rev()
            .find(|s| s.frame == frame)
        else {
            self.status = "Nothing to fill: draw a closed shape on this frame first.".into();
            return;
        };
        if src.points.len() < 3 {
            self.status = "Nothing to fill: the last stroke needs at least 3 points to make a shape.".into();
            return;
        }
        let spacing = src.width;
        let path = crate::paint::polyline_fill_path(&src.points, spacing);
        if path.len() < 2 {
            self.status = "Nothing to fill: the shape is too small or flat to hold a fill.".into();
            return;
        }
        let color = src.color;
        let n_points = path.len();
        self.snapshot();
        if let Some(s) = self
            .project
            .strokes
            .iter_mut()
            .rev()
            .find(|s| s.frame == frame)
        {
            s.fill = Some(color);
        }
        self.project.strokes.push(crate::model::Stroke {
            frame,
            points: path,
            color,
            width: spacing,
            widths: Vec::new(),
            fill: Some(color),
            spray: None,
        });
        self.status = format!("Shape filled with a scanline fill ({n_points} points).");
    }

    /// Erase strokes on this frame that lie mostly (a strict majority
    /// of their points) inside the lasso polygon `poly`.
    pub fn lasso_erase(&mut self, poly: &[V2]) {
        if poly.len() < 3 {
            self.status = "Lasso needs at least 3 points; nothing erased.".into();
            return;
        }
        let frame = self.current;
        let mostly_inside = |s: &crate::model::Stroke| {
            s.frame == frame
                && !s.points.is_empty()
                && s.points
                    .iter()
                    .filter(|p| crate::paint::point_in_polygon(**p, poly))
                    .count()
                    * 2
                    > s.points.len()
        };
        let n = self.project.strokes.iter().filter(|s| mostly_inside(s)).count();
        if n == 0 {
            self.status = "Lasso erased nothing: no stroke lies mostly inside it.".into();
            return;
        }
        self.snapshot();
        self.project.strokes.retain(|s| !mostly_inside(s));
        self.status = format!("Lasso erased {n} stroke(s).");
    }

    /// Wave 12: mirror every stroke on the current frame across
    /// the stage's vertical centre line (x = stage width / 2), as
    /// one undo step. Only the points change — colour, width,
    /// per-point widths and fill stay as drawn — and strokes on
    /// other frames are left alone. Mirroring twice restores the
    /// original drawing exactly. A frame with no strokes is a
    /// no-op (no snapshot).
    pub fn mirror_strokes_x(&mut self) {
        let frame = self.current;
        let n = self
            .project
            .strokes
            .iter()
            .filter(|s| s.frame == frame)
            .count();
        if n == 0 {
            self.status = format!("No strokes to mirror on frame {}.", frame + 1);
            return;
        }
        let cx = self.project.stage[0] / 2.0;
        self.snapshot();
        for s in &mut self.project.strokes {
            if s.frame == frame {
                s.points = crate::paint::mirror_points_x(&s.points, cx);
            }
        }
        self.status = format!(
            "Mirrored {n} stroke(s) on frame {} across the stage centre.",
            frame + 1
        );
    }

    /// Shared stamp construction: a generated outline becomes
    /// ONE stroke on the current frame, built exactly like a
    /// drawn stroke (`begin_draw`): the selected figure's
    /// colour, the active brush's width recorded per point (a
    /// drawn stroke carries a width for every point), and no
    /// fill. One snapshot per stamp.
    fn stamp_points(&mut self, points: Vec<V2>, what: &str) {
        let brush = self.active_brush();
        let color = self.project.figures[self.selected_figure].color;
        let n = points.len();
        self.snapshot();
        self.project.strokes.push(crate::model::Stroke {
            frame: self.current,
            points,
            color,
            width: brush.width,
            widths: vec![brush.width; n],
            fill: None,
            spray: None,
        });
        self.status = format!("Stamped {what} on frame {} ({n} points).", self.current + 1);
    }

    /// Wave 12: stamp a regular pentagon (radius 60, one vertex
    /// pointing up) at the stage centre on the current frame.
    pub fn stamp_polygon(&mut self) {
        let c = [self.project.stage[0] / 2.0, self.project.stage[1] / 2.0];
        let points = crate::paint::regular_polygon_points(c, 60.0, 5, -90.0);
        self.stamp_points(points, "a pentagon");
    }

    /// Wave 12: stamp a five-pointed star (outer radius 60,
    /// inner radius 26, one point up) at the stage centre on
    /// the current frame.
    pub fn stamp_star(&mut self) {
        let c = [self.project.stage[0] / 2.0, self.project.stage[1] / 2.0];
        let points = crate::paint::star_points(c, 60.0, 26.0, 5, -90.0);
        self.stamp_points(points, "a star");
    }

    /// Wave 12: stamp a horizontal arrow at the stage centre on
    /// the current frame, pointing right: the shaft runs from
    /// 70 units left of centre to 70 units right, with a
    /// 22-long head.
    pub fn stamp_arrow(&mut self) {
        let cx = self.project.stage[0] / 2.0;
        let cy = self.project.stage[1] / 2.0;
        let points = crate::paint::arrow_points([cx - 70.0, cy], [cx + 70.0, cy], 22.0);
        self.stamp_points(points, "an arrow");
    }

    // ---- Draw tools (wave 7) ----

    /// The active brush preset from `paint::brush_library`
    /// (`brush_index` clamped, so a stale index can never panic).
    pub fn active_brush(&self) -> crate::paint::Brush {
        let lib = crate::paint::brush_library();
        lib[self.brush_index.min(lib.len() - 1)]
    }

    /// Eraser radius for the active brush: twice its width, so a
    /// fatter brush also erases a fatter swath.
    fn eraser_radius(&self) -> f32 {
        self.active_brush().width * 2.0
    }

    /// Cycle to the next brush preset in the library (wrapping).
    /// Applies to whichever tool is active: the brush paints with
    /// it, the eraser is sized by it.
    pub fn cycle_brush(&mut self) {
        let lib = crate::paint::brush_library();
        self.brush_index = (self.brush_index + 1) % lib.len();
        let b = self.active_brush();
        self.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
    }

    /// True when the eraser is the active draw tool (the app's
    /// tool-toggle button highlights and labels itself from this,
    /// so it never has to name `DrawTool` itself).
    pub fn draw_tool_is_eraser(&self) -> bool {
        self.draw_tool == DrawTool::Eraser
    }

    /// Switch the draw tool between brush and eraser.
    pub fn toggle_draw_tool(&mut self) {
        self.draw_tool = match self.draw_tool {
            DrawTool::Brush => DrawTool::Eraser,
            DrawTool::Eraser => DrawTool::Brush,
        };
        self.status = match self.draw_tool {
            DrawTool::Eraser => "Draw tool: Eraser.".into(),
            DrawTool::Brush => format!("Draw tool: Brush ({}).", self.active_brush().name),
        };
    }

    // ---- Audio ----

    pub fn add_audio_clip(&mut self) {
        let Some(path) = self.first_file_with("audio", &["wav", "mp3", "ogg"]) else {
            self.status = "Put an audio file (.wav/.mp3/.ogg) in ./audio/ first.".into();
            return;
        };
        self.snapshot();
        self.project.audio.push(crate::model::AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: self.current,
            gain: 1.0,
        });
        self.status = format!("Audio clip added at frame {}: {}", self.current + 1, path.display());
    }

    // ---- Wave 6 audio editing ----

    /// Path for an audio edit product: same directory as `source`,
    /// with the source file stem plus `suffix` (for example
    /// `.norm.wav`). The suffix always changes the file name, so an
    /// edit product cannot accidentally be its own source.
    fn audio_derived_path(source: &PathBuf, suffix: &str) -> PathBuf {
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio".into());
        let mut out = source.clone();
        out.set_file_name(format!("{stem}{suffix}"));
        out
    }

    /// Normalise audio clip `clip_idx` to a peak of 0.9.
    ///
    /// The normalised audio is written beside the source as
    /// `<stem>.norm.wav`; the source file is never overwritten.
    /// Only after that file has been written does the clip point to
    /// it. A missing index or a source that cannot be read as a WAV
    /// sets an error status and leaves the project unchanged.
    pub fn audio_normalize(&mut self, clip_idx: usize) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio normalise failed: no clip {}.", clip_idx + 1);
            return;
        };
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio normalise failed: {e}");
                return;
            }
        };
        let gain = crate::audio_edit::normalize(&mut wav, 0.9);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".norm.wav");
        if out == source {
            self.status = "Audio normalise failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio normalise failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Audio clip {} normalised to peak 0.90 (gain {gain:.2}x): {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Trim audio clip `clip_idx` to `[start_secs, end_secs)` and
    /// write the result beside the source as `<stem>.trim.wav`.
    ///
    /// When `start_secs` is positive, the clip's `start_frame`
    /// advances by `round(start_secs * fps)`. The trimmed head held
    /// the clip's earliest audible content, so the remaining content
    /// originally sounded that much later; shifting the placement by
    /// the same amount preserves its position on the timeline. A
    /// non-positive start does not move the clip. An empty trim
    /// range is an error and leaves the project unchanged.
    pub fn audio_trim(&mut self, clip_idx: usize, start_secs: f32, end_secs: f32) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio trim failed: no clip {}.", clip_idx + 1);
            return;
        };
        if !start_secs.is_finite() || !end_secs.is_finite() {
            self.status = "Audio trim failed: trim times must be finite.".into();
            return;
        }
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio trim failed: {e}");
                return;
            }
        };
        let trimmed = crate::audio_edit::trim(&wav, start_secs, end_secs);
        if trimmed.frames() == 0 {
            self.status = "Audio trim failed: the trim range is empty.".into();
            return;
        }
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".trim.wav");
        if out == source {
            self.status = "Audio trim failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&trimmed)) {
            self.status = format!("Audio trim failed: could not write {}: {e}", out.display());
            return;
        }
        let fps = self.project.fps.max(1.0);
        let shift = if start_secs > 0.0 {
            (start_secs * fps).round().max(0.0) as usize
        } else {
            0
        };
        self.snapshot();
        let clip = &mut self.project.audio[clip_idx];
        clip.path = out.to_string_lossy().into_owned();
        clip.start_frame = clip.start_frame.saturating_add(shift);
        self.status = format!(
            "Audio clip {} trimmed to {:.2}s; starts at frame {}.",
            clip_idx + 1,
            trimmed.duration_secs(),
            clip.start_frame + 1
        );
    }

    /// Split audio clip `clip_idx` at timeline frame `at_frame`.
    ///
    /// `crate::audio_edit::split_clip` splits the timeline placement
    /// and `crate::audio_edit::split_wav` cuts the actual audio.
    /// Both halves are materialised as real files, `<stem>.a.wav`
    /// and `<stem>.b.wav`, and the second clip points at the `.b`
    /// file. Materialising the tail is what makes playback of the
    /// second clip correct: `AudioClip` has no source-offset field,
    /// so a second clip pointing at the original file would restart
    /// the audio from its beginning. The second clip is inserted
    /// immediately after the first. A split point that is not
    /// strictly inside the clip is an error and leaves the project
    /// unchanged.
    pub fn audio_split(&mut self, clip_idx: usize, at_frame: usize) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio split failed: no clip {}.", clip_idx + 1);
            return;
        };
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio split failed: {e}");
                return;
            }
        };
        let fps = self.project.fps.max(1.0);
        let dur_frames = (wav.duration_secs() * fps).round() as usize;
        let Some((mut first, mut second)) =
            crate::audio_edit::split_clip(&clip, at_frame, dur_frames)
        else {
            self.status = format!(
                "Audio split failed: frame {} is not inside clip {}.",
                at_frame + 1,
                clip_idx + 1
            );
            return;
        };
        let split_secs = (at_frame - clip.start_frame) as f32 / fps;
        let (first_wav, second_wav) = crate::audio_edit::split_wav(&wav, split_secs);
        if first_wav.frames() == 0 || second_wav.frames() == 0 {
            self.status = "Audio split failed: the split point leaves an empty half.".into();
            return;
        }
        let source = PathBuf::from(&clip.path);
        let first_path = Self::audio_derived_path(&source, ".a.wav");
        let second_path = Self::audio_derived_path(&source, ".b.wav");
        if let Err(e) = std::fs::write(&first_path, crate::audio_edit::write_wav(&first_wav)) {
            self.status = format!(
                "Audio split failed: could not write {}: {e}",
                first_path.display()
            );
            return;
        }
        if let Err(e) = std::fs::write(&second_path, crate::audio_edit::write_wav(&second_wav)) {
            self.status = format!(
                "Audio split failed: could not write {}: {e}",
                second_path.display()
            );
            return;
        }
        first.path = first_path.to_string_lossy().into_owned();
        second.path = second_path.to_string_lossy().into_owned();
        self.snapshot();
        self.project.audio[clip_idx] = first;
        self.project.audio.insert(clip_idx + 1, second);
        self.status = format!(
            "Audio clip {} split at frame {} into {} and {}.",
            clip_idx + 1,
            at_frame + 1,
            first_path.display(),
            second_path.display()
        );
    }

    /// Crossfade audio clip `clip_idx` into the following clip and
    /// replace the pair with one clip at the first clip's start.
    ///
    /// If the clips overlap on the timeline, the fade lasts for the
    /// overlap, preserving the second clip's timeline position in
    /// the joined file. If they are merely adjacent, the fade is
    /// 1.0 seconds, capped by the shorter clip's duration; creating
    /// that overlap necessarily makes the joined file shorter than
    /// the two clips laid end to end. Clips separated by a gap, an
    /// overlap too long for either source, or a source that cannot
    /// be read as a WAV are errors and leave the project unchanged.
    /// The joined audio is written beside the first source as
    /// `<stem>.xfade.wav`.
    pub fn audio_crossfade(&mut self, clip_idx: usize) {
        if clip_idx + 1 >= self.project.audio.len() {
            self.status = format!(
                "Audio crossfade failed: clip {} has no following clip.",
                clip_idx + 1
            );
            return;
        }
        let first = self.project.audio[clip_idx].clone();
        let second = self.project.audio[clip_idx + 1].clone();
        let first_wav = match crate::audio_edit::read_wav(&first.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        let second_wav = match crate::audio_edit::read_wav(&second.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        if first_wav.frames() == 0 || second_wav.frames() == 0 {
            self.status = "Audio crossfade failed: a clip is empty.".into();
            return;
        }
        let fps = self.project.fps.max(1.0);
        let first_duration = first_wav.duration_secs();
        let second_duration = second_wav.duration_secs();
        let first_start = first.start_frame as f32 / fps;
        let second_start = second.start_frame as f32 / fps;
        let overlap = first_start + first_duration - second_start;
        // Starts and ends are frame-quantised; half a frame of
        // slack keeps an exactly adjacent pair from looking gapped
        // after floating-point conversion.
        let adjacency_tolerance = 0.5 / fps;
        let fade = if overlap > adjacency_tolerance {
            overlap
        } else if overlap >= -adjacency_tolerance {
            1.0_f32.min(first_duration).min(second_duration)
        } else {
            self.status = format!(
                "Audio crossfade failed: clips {} and {} are separated by a {:.2}s gap.",
                clip_idx + 1,
                clip_idx + 2,
                -overlap
            );
            return;
        };
        if fade <= 0.0 || fade > first_duration || fade > second_duration {
            self.status =
                "Audio crossfade failed: the clips do not overlap by a usable amount.".into();
            return;
        }
        let mixed = match crate::audio_edit::crossfade_mix(&first_wav, &second_wav, fade) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        let source = PathBuf::from(&first.path);
        let out = Self::audio_derived_path(&source, ".xfade.wav");
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&mixed)) {
            self.status = format!(
                "Audio crossfade failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.project.audio.remove(clip_idx + 1);
        self.status = format!(
            "Audio clips {} and {} crossfaded ({fade:.2}s) into {}.",
            clip_idx + 1,
            clip_idx + 2,
            out.display()
        );
    }

    // ---- Wave 11 audio effects (last clip) ----

    /// Wave 11: apply a feedback echo (0.18 s delay, 0.35
    /// feedback) to the LAST audio clip. Follows the
    /// `audio_normalize` pattern: the processed audio is written
    /// beside the source as `<stem>.echo.wav`, the source file is
    /// never overwritten, and only after the write does the clip
    /// point to the new file (behind one undo snapshot).
    pub fn audio_echo_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio echo failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio echo failed: {e}");
                return;
            }
        };
        crate::audio_edit::echo(&mut wav, 0.18, 0.35);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".echo.wav");
        if out == source {
            self.status = "Audio echo failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!("Audio echo failed: could not write {}: {e}", out.display());
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Echo applied to audio clip {} (0.18s delay, 35% feedback): {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 11: apply a 1200 Hz low-pass filter to the LAST
    /// audio clip. Same materialise-and-repoint pattern as
    /// `audio_echo_last`; the product is `<stem>.lowpass.wav`.
    pub fn audio_lowpass_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio low-pass failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio low-pass failed: {e}");
                return;
            }
        };
        crate::audio_edit::lowpass(&mut wav, 1200.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".lowpass.wav");
        if out == source {
            self.status = "Audio low-pass failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio low-pass failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Low-pass filter (1200 Hz) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 11: trim leading and trailing silence (samples below
    /// 0.02) from the LAST audio clip. Same materialise-and-
    /// repoint pattern as `audio_echo_last`; the product is
    /// `<stem>.silence.wav`. When the clip has no silence at
    /// either edge there is nothing to write: no file is created,
    /// no snapshot is taken, and the status says so.
    pub fn audio_trim_silence_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio trim silence failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio trim silence failed: {e}");
                return;
            }
        };
        let (lead, trail) = crate::audio_edit::trim_silence(&mut wav, 0.02);
        if lead + trail == 0 {
            self.status = format!("No silence to trim from audio clip {}.", clip_idx + 1);
            return;
        }
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".silence.wav");
        if out == source {
            self.status = "Audio trim silence failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio trim silence failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Trimmed {} silent frame(s) ({} at the start, {} at the end) from audio clip {}: {}.",
            lead + trail,
            lead,
            trail,
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 12: pitch-shift the LAST audio clip up 2 semitones
    /// (a rate shift — pitch and duration change together, the
    /// stored sample rate is untouched). Same materialise-and-
    /// repoint pattern as `audio_echo_last`; the product is
    /// `<stem>.pitch.wav` and the source file is never
    /// overwritten.
    pub fn audio_pitch_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio pitch shift failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio pitch shift failed: {e}");
                return;
            }
        };
        crate::audio_edit::pitch_shift(&mut wav, 2.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".pitch.wav");
        if out == source {
            self.status = "Audio pitch shift failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio pitch shift failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Pitch shift (+2 semitones) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 12: run a compressor (threshold 0.5, ratio 4.0)
    /// over the LAST audio clip. Same materialise-and-repoint
    /// pattern as `audio_echo_last`; the product is
    /// `<stem>.comp.wav` and the source file is never
    /// overwritten.
    pub fn audio_compress_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio compress failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio compress failed: {e}");
                return;
            }
        };
        crate::audio_edit::compress(&mut wav, 0.5, 4.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".comp.wav");
        if out == source {
            self.status = "Audio compress failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio compress failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Compressor (threshold 0.5, ratio 4.0) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 13: split the LAST audio clip wherever silence
    /// (below 0.02) lasts at least a quarter second, using
    /// `audio_edit::split_at_silences` to find the sounding
    /// ranges. Each range is materialised beside the source as
    /// `<stem>.seg1.wav`, `<stem>.seg2.wav`, ... — a segment
    /// clip pointing at the original file would restart the
    /// audio from its beginning, because `AudioClip` has no
    /// source-offset field (the same reason `audio_split`
    /// materialises its tail). The first segment replaces the
    /// last clip and the remaining segments are inserted
    /// immediately after it, each starting at the original
    /// start plus `round(range_start_secs * fps)` — the shift
    /// convention `audio_trim` uses for a trimmed head — and
    /// keeping the original clip's gain. One snapshot, taken
    /// only after every segment file is written. Fewer than
    /// two ranges (no qualifying silence) is a no-op: no files
    /// are written and no snapshot is taken.
    pub fn audio_split_at_silences_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio split at silences failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio split at silences failed: {e}");
                return;
            }
        };
        let rate = wav.sample_rate;
        let min_silence = (rate as f32 * 0.25) as usize;
        let ranges = crate::audio_edit::split_at_silences(&wav, 0.02, min_silence);
        if ranges.len() < 2 {
            self.status = format!(
                "Audio clip {} has no silence of 0.25s or more to split at.",
                clip_idx + 1
            );
            return;
        }
        let source = PathBuf::from(&clip.path);
        let ch = wav.channels as usize;
        let fps = self.project.fps.max(1.0);
        let mut segments: Vec<crate::model::AudioClip> =
            Vec::with_capacity(ranges.len());
        for (i, &(start, end)) in ranges.iter().enumerate() {
            let seg = crate::audio_edit::WavData {
                sample_rate: wav.sample_rate,
                channels: wav.channels,
                samples: wav.samples[start * ch..end * ch].to_vec(),
            };
            let out = Self::audio_derived_path(&source, &format!(".seg{}.wav", i + 1));
            if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&seg)) {
                self.status = format!(
                    "Audio split at silences failed: could not write {}: {e}",
                    out.display()
                );
                return;
            }
            let start_secs = start as f32 / rate.max(1) as f32;
            let shift = (start_secs * fps).round().max(0.0) as usize;
            segments.push(crate::model::AudioClip {
                path: out.to_string_lossy().into_owned(),
                start_frame: clip.start_frame.saturating_add(shift),
                gain: clip.gain,
            });
        }
        let n = segments.len();
        self.snapshot();
        // The first segment replaces the split clip; the rest
        // are inserted immediately after it, in order (the
        // placement `audio_split` uses for its second half).
        let mut segments = segments.into_iter();
        self.project.audio[clip_idx] =
            segments.next().expect("at least two ranges were checked");
        for (j, seg) in segments.enumerate() {
            self.project.audio.insert(clip_idx + 1 + j, seg);
        }
        self.status = format!(
            "Audio clip {} split at silences into {n} clips.",
            clip_idx + 1
        );
    }

    // ---- Movieclips ----

    pub fn make_clip_from_figure(&mut self) {
        self.snapshot();
        let fi = self.selected_figure;
        let fig = &self.project.figures[fi];
        let frames: Vec<crate::model::Pose> = self
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].clone())
            .collect();
        let name = format!("{} clip", fig.name);
        self.project.clips.push(crate::model::ClipDef {
            name: name.clone(),
            def: fig.def.clone(),
            frames,
            loop_mode: crate::model::ClipLoop::Loop,
        });
        let ci = self.project.clips.len() - 1;
        self.project.figures[fi].clip = Some(ci);
        self.status = format!("{name} created; this figure now loops its own timeline.");
    }

    pub fn insert_clip_figure(&mut self) {
        if self.project.clips.is_empty() {
            self.status = "No movieclips yet — select a figure and use Make clip.".into();
            return;
        }
        self.snapshot();
        let ci = 0;
        let clip = self.project.clips[ci].clone();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[(idx + 3) % FIGURE_COLORS.len()];
        self.project
            .add_figure_with(clip.def.clone(), &clip.name, color, [380.0 + 40.0 * idx as f32, 450.0]);
        self.project.figures[idx].clip = Some(ci);
        self.selected_figure = idx;
        self.status = format!("Inserted movieclip {}.", clip.name);
    }

    // ---- Wave 7: movieclip editing session ----
    //
    // A session scopes the ops below to one ClipDef's own frames
    // instead of the project timeline: they mutate
    // `project.clips[session.clip]` only, never the project frames.
    // Structural ops (add/delete frame, creating a clip) snapshot
    // for undo; pose tweaks (angle/root/goto) do not — the same
    // rule as dragging, where the gesture snapshots, not each move.

    /// Enter the editing session for clip `clip_idx`, starting at
    /// its first frame. Returns false (with a status message) when
    /// the index is invalid or the clip has no frames to edit.
    pub fn enter_clip(&mut self, clip_idx: usize) -> bool {
        let Some(clip) = self.project.clips.get(clip_idx) else {
            self.status = format!("No movieclip {} to edit.", clip_idx + 1);
            return false;
        };
        if clip.frames.is_empty() {
            self.status = format!("Movieclip {} has no frames to edit.", clip.name);
            return false;
        }
        let name = clip.name.clone();
        let count = clip.frames.len();
        self.clip_edit = Some(ClipEditSession { clip: clip_idx, frame: 0 });
        self.status = format!("Editing movieclip {name} ({count} frames).");
        true
    }

    /// Leave the editing session. The clip keeps its edits.
    pub fn exit_clip(&mut self) {
        if self.clip_edit.take().is_some() {
            self.status = "Movieclip editing closed.".into();
        } else {
            self.status = "Not editing a movieclip.".into();
        }
    }

    /// Validate the live session: returns (clip index, frame index)
    /// with the frame clamped into range. A session whose clip has
    /// vanished (e.g. after an undo) is dropped with a status
    /// message. Callers first check `clip_edit.is_none()` to report
    /// the no-session case themselves.
    fn clip_session(&mut self) -> Option<(usize, usize)> {
        let session = self.clip_edit?;
        let Some(clip) = self.project.clips.get(session.clip) else {
            self.clip_edit = None;
            self.status = "Movieclip is gone — editing session closed.".into();
            return None;
        };
        if clip.frames.is_empty() {
            self.status = "Movieclip has no frames to edit.".into();
            return None;
        }
        let frame = session.frame.min(clip.frames.len() - 1);
        if frame != session.frame {
            if let Some(s) = self.clip_edit.as_mut() {
                s.frame = frame;
            }
        }
        Some((session.clip, frame))
    }

    /// Keep clip frame `fi` of clip `ci` node-count wide: angle
    /// vecs are truncated or padded with rest angles.
    fn clip_fit_frame(&mut self, ci: usize, fi: usize) {
        let clip = &mut self.project.clips[ci];
        let n = clip.def.nodes.len();
        let cur = clip.frames[fi].angles.len();
        if cur < n {
            let extra: Vec<f32> = clip.def.nodes[cur..]
                .iter()
                .map(|node| node.rest_angle)
                .collect();
            clip.frames[fi].angles.extend(extra);
        }
        clip.frames[fi].angles.truncate(n);
    }

    /// Append a frame to the edited clip, duplicating its last
    /// pose, and move the session onto the new frame.
    pub fn clip_add_frame(&mut self) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, _)) = self.clip_session() else { return };
        self.snapshot();
        let pose = self.project.clips[ci]
            .frames
            .last()
            .expect("clip_session guarantees frames")
            .clone();
        self.project.clips[ci].frames.push(pose);
        let idx = self.project.clips[ci].frames.len() - 1;
        self.clip_fit_frame(ci, idx);
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = idx;
        }
        self.status = format!("Movieclip frame {} added.", idx + 1);
    }

    /// Delete the session's current clip frame. Refused when it is
    /// the clip's only frame.
    pub fn clip_delete_frame(&mut self) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        if self.project.clips[ci].frames.len() <= 1 {
            self.status = "A movieclip needs at least one frame.".into();
            return;
        }
        self.snapshot();
        self.project.clips[ci].frames.remove(fi);
        let last = self.project.clips[ci].frames.len() - 1;
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = s.frame.min(last);
        }
        self.status = format!("Movieclip frame {} deleted.", fi + 1);
    }

    /// Move the session to clip frame `idx` (clamped to the
    /// clip's last frame).
    pub fn clip_goto(&mut self, idx: usize) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, _)) = self.clip_session() else { return };
        let last = self.project.clips[ci].frames.len() - 1;
        let target = idx.min(last);
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = target;
        }
        self.status = format!("Movieclip frame {}.", target + 1);
    }

    /// Set joint `node`'s angle on the session frame's pose.
    /// Out-of-range joints are refused with a status message.
    pub fn clip_set_angle(&mut self, node: usize, degrees: f32) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        if node >= self.project.clips[ci].def.nodes.len() {
            self.status = format!("Movieclip has no joint {}.", node + 1);
            return;
        }
        self.clip_fit_frame(ci, fi);
        self.project.clips[ci].frames[fi].angles[node] = degrees;
    }

    /// Set the root position on the session frame's pose.
    pub fn clip_set_root(&mut self, root: V2) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        self.project.clips[ci].frames[fi].root = root;
    }

    /// Joint positions of the edited clip at the session frame —
    /// what a canvas view of the session would draw. Empty when
    /// no session is active.
    pub fn clip_positions(&self) -> Vec<V2> {
        let Some(session) = self.clip_edit else { return Vec::new() };
        let Some(clip) = self.project.clips.get(session.clip) else {
            return Vec::new();
        };
        let frame = session.frame.min(clip.frames.len().saturating_sub(1));
        let Some(pose) = clip.frames.get(frame) else {
            return Vec::new();
        };
        evaluate(&clip.def, pose)
    }

    /// Build a movieclip from figure `fi`: the clip takes the
    /// figure's definition and its pose from every project frame,
    /// is appended to `project.clips`, and the instance is pointed
    /// at it, so the figure loops the clip from now on. Because
    /// the clip frames ARE the timeline poses, the first loop pass
    /// is visually identical to the timeline it replaces (after
    /// that the loop repeats with the clip's length, independent
    /// of the timeline). Returns the new clip's index; this is
    /// the parameterised sibling of `make_clip_from_figure`.
    pub fn create_clip_from_figure(&mut self, fi: usize) -> Option<usize> {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to make a clip from.", fi + 1);
            return None;
        }
        if self.project.frames.is_empty() {
            self.status = "No frames to build a movieclip from.".into();
            return None;
        }
        self.snapshot();
        let (name, def) = {
            let fig = &self.project.figures[fi];
            (format!("{} clip", fig.name), fig.def.clone())
        };
        // Poses for `fi` across the timeline; a frame missing the
        // pose (should not happen — the editor keeps pose vecs
        // figure-wide) falls back to the previous pose, or to the
        // rest pose at stage centre for the first frame.
        let mut fallback = def.rest_pose([self.project.stage[0] / 2.0, self.project.stage[1] / 2.0]);
        let mut frames: Vec<crate::model::Pose> = Vec::with_capacity(self.project.frames.len());
        for frame in &self.project.frames {
            let pose = frame.poses.get(fi).cloned().unwrap_or_else(|| fallback.clone());
            fallback = pose.clone();
            frames.push(pose);
        }
        self.project.clips.push(crate::model::ClipDef {
            name: name.clone(),
            def,
            frames,
            loop_mode: crate::model::ClipLoop::Loop,
        });
        let ci = self.project.clips.len() - 1;
        for f in 0..self.project.clips[ci].frames.len() {
            self.clip_fit_frame(ci, f);
        }
        self.project.figures[fi].clip = Some(ci);
        self.status = format!("{name} created; figure {} now loops it.", fi + 1);
        Some(ci)
    }

    /// Wave 10: cycle a clip's loop mode — Loop → PingPong →
    /// Once → Loop — as one undo step. `model::poses_at` honours
    /// the mode when a figure plays the clip back.
    pub fn cycle_clip_loop(&mut self, index: usize) {
        if index >= self.project.clips.len() {
            self.status = "No such clip.".into();
            return;
        }
        self.snapshot();
        let clip = &mut self.project.clips[index];
        clip.loop_mode = match clip.loop_mode {
            crate::model::ClipLoop::Loop => crate::model::ClipLoop::PingPong,
            crate::model::ClipLoop::PingPong => crate::model::ClipLoop::Once,
            crate::model::ClipLoop::Once => crate::model::ClipLoop::Loop,
        };
        self.status = format!(
            "Clip loop: {}.",
            match clip.loop_mode {
                crate::model::ClipLoop::Loop => "loop",
                crate::model::ClipLoop::PingPong => "ping-pong",
                crate::model::ClipLoop::Once => "once",
            }
        );
    }

    // ---- Named projects ----

    pub fn save_named_project(&mut self) {
        let slug = self
            .project
            .name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-");
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("projects");
        let path = dir.join(format!("{slug}.json"));
        let result = std::fs::create_dir_all(&dir)
            .and_then(|_| serde_json::to_string_pretty(&self.project).map_err(std::io::Error::other))
            .and_then(|text| std::fs::write(&path, text));
        match result {
            Ok(()) => {
                self.project_files = Self::list_project_files();
                self.status = format!("Project saved to {}", path.display());
            }
            Err(e) => self.status = format!("Project save failed: {e}"),
        }
    }

    pub fn cycle_project_file(&mut self) {
        if self.project_files.is_empty() {
            self.project_files = Self::list_project_files();
        }
        if !self.project_files.is_empty() {
            self.project_index = (self.project_index + 1) % self.project_files.len();
            self.status = format!(
                "Project: {}",
                self.project_files[self.project_index].display()
            );
        } else {
            self.status = "No saved projects in ./projects/ yet.".into();
        }
    }

    pub fn open_project_file(&mut self) {
        let Some(path) = self.project_files.get(self.project_index).cloned() else {
            self.status = "No project selected.".into();
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Project>(&s).map_err(|e| e.to_string()))
        {
            Ok(project) => {
                self.project = project;
                self.current = 0;
                self.playhead = 0.0;
                self.selected_figure = 0;
                self.selected_node = None;
                // Mirror the dialog-open op: a solo index
                // from the previous project must not hide
                // every figure in the newly opened one.
                self.solo = None;
                self.status = format!("Opened {}", path.display());
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    pub fn cycle_onion_frames(&mut self) {
        self.onion_frames = self.onion_frames % 3 + 1;
        self.status = format!("Onion skin: {} previous frame(s).", self.onion_frames);
    }

    pub fn export_mp4(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("animation.mp4");
        match render_mp4(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame MP4 to {}", path.display()),
            Err(e) => self.status = format!("MP4 export failed: {e}"),
        }
    }

    pub fn export_webm(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("animation.webm");
        match render_webm(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame WebM."),
            Err(e) => self.status = format!("WebM export failed: {e}"),
        }
    }

    pub fn export_svg(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("svg");
        match crate::svg::export_svg(&self.project, &dir) {
            Ok(n) => self.status = format!("Exported {n} SVG frames to {}", dir.display()),
            Err(e) => self.status = format!("SVG export failed: {e}"),
        }
    }

    pub fn export_sheet(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("spritesheet.png");
        match render_sheet(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame spritesheet."),
            Err(e) => self.status = format!("Spritesheet failed: {e}"),
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

    // ---- 3D figures (fig3d session, manipulation, bake to 2D) ----

    /// Enter 3D mode: load the 3D humanoid at rest with the default
    /// orbit camera. If a 3D session is already active it is kept
    /// as-is (pose and camera preserved).
    pub fn enter_3d(&mut self) {
        if self.fig3.is_some() {
            self.status = "3D mode is already active.".into();
            return;
        }
        let figure = crate::fig3d::humanoid3();
        let pose = crate::fig3d::Pose3::rest(&figure, [0.0, 0.0, 0.0]);
        self.fig3 = Some(crate::fig3d::Fig3State {
            figure,
            pose,
            camera: crate::fig3d::default_camera(),
            selected: 0,
            pose_b: None,
        });
        self.status = "3D mode: posing the 3D humanoid. Bake turns it into a 2D figure.".into();
    }

    /// Leave 3D mode, discarding the 3D session. Figures already
    /// baked into the project stay.
    pub fn exit_3d(&mut self) {
        if self.fig3.take().is_some() {
            self.status = "Exited 3D mode.".into();
        } else {
            self.status = "Not in 3D mode.".into();
        }
    }

    /// Cycle the selected 3D joint to the next node.
    pub fn fig3_select_next(&mut self) {
        let msg = match &mut self.fig3 {
            Some(st) if !st.figure.nodes.is_empty() => {
                st.selected = (st.selected + 1) % st.figure.nodes.len();
                format!("3D joint: {}.", st.figure.nodes[st.selected].name)
            }
            _ => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Rotate the selected 3D joint by `delta_deg` around one Euler
    /// axis (0 = X, 1 = Y, 2 = Z; order documented in fig3d).
    pub fn fig3_rotate(&mut self, axis: usize, delta_deg: f32) {
        let msg = match &mut self.fig3 {
            Some(st) if axis < 3 && !st.figure.nodes.is_empty() => {
                let sel = st.selected.min(st.figure.nodes.len() - 1);
                while st.pose.rotations.len() < st.figure.nodes.len() {
                    st.pose.rotations.push([0.0; 3]);
                }
                st.pose.rotations[sel][axis] += delta_deg;
                format!(
                    "3D {} rotated to {:.0} deg on axis {}.",
                    st.figure.nodes[sel].name,
                    st.pose.rotations[sel][axis],
                    "XYZ".chars().nth(axis).unwrap()
                )
            }
            Some(_) => "Rotation axis must be 0 (X), 1 (Y) or 2 (Z).".into(),
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Orbit the 3D camera by the given yaw/pitch deltas in degrees.
    /// Pitch clamps to +-89.9, the same clamp the projector applies,
    /// so the stored camera never degenerates.
    pub fn fig3_orbit(&mut self, dyaw: f32, dpitch: f32) {
        let msg = match &mut self.fig3 {
            Some(st) => {
                st.camera.yaw += dyaw;
                st.camera.pitch = (st.camera.pitch + dpitch).clamp(-89.9, 89.9);
                format!(
                    "3D camera: yaw {:.0}, pitch {:.0}.",
                    st.camera.yaw, st.camera.pitch
                )
            }
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Relative 2D angle of every node in one frame of 3D bake
    /// nodes: each node's baked absolute direction minus its
    /// parent's (0 for the root). Shared by the figure-definition
    /// build and the per-frame pose build so the two can never
    /// drift apart.
    fn fig3_relative_angles(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> Vec<f32> {
        baked
            .iter()
            .enumerate()
            .map(|(i, bn)| {
                let parent_abs = fig.nodes[i]
                    .parent
                    .map(|p| baked[p].abs_angle)
                    .unwrap_or(0.0);
                crate::model::norm_deg(bn.abs_angle - parent_abs)
            })
            .collect()
    }

    /// Build the 2D figure definition for one frame of 3D bake
    /// nodes (origin node + one node per 3D joint, parent structure
    /// preserved, thickness scaled by projected scale — the same
    /// conversion the file importers use).
    fn fig3_bake_def(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> FigureDef {
        let rel = Self::fig3_relative_angles(fig, baked);
        let nodes = fig
            .nodes
            .iter()
            .zip(baked.iter().zip(rel.iter()))
            .map(|(n3, (bn, &rest_angle))| NodeDef {
                name: n3.name.clone(),
                parent: n3.parent,
                length: bn.length,
                rest_angle,
                head: n3.head,
                bend: 1.0,
                thickness: bn.thickness,
                color: None,
                ik_end: false,
                shape: crate::model::Shape::Line,
                style: crate::model::NodeStyle::default(),
                media: None,
            })
            .collect();
        FigureDef { nodes }
    }

    /// The 2D pose one frame of 3D bake nodes represents in a
    /// figure whose definition came from [`Self::fig3_bake_def`]:
    /// root at the baked root position, relative angles exactly as
    /// in the definition. Because the 2D evaluator accumulates
    /// relative angles along the tree, the evaluated joints
    /// reproduce the baked points whenever this frame's projected
    /// segment lengths equal the definition's — exactly true for
    /// the bake that built the definition, and whenever the motion
    /// stays in the projection plane.
    fn fig3_bake_pose(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> crate::model::Pose {
        crate::model::Pose {
            root: baked.first().map(|b| b.xy).unwrap_or([0.0, 0.0]),
            angles: Self::fig3_relative_angles(fig, baked),
        }
    }

    /// Bake the current 3D pose into a new 2D figure in the project.
    /// Projected segment lengths and directions become the figure
    /// definition (see [`Self::fig3_bake_def`]). The pose is
    /// stored as the definition's rest pose, so the held-pose
    /// convention of `add_figure_with` puts it on every frame.
    pub fn fig3_bake(&mut self) {
        let built = self.fig3.as_ref().map(|st| {
            let baked = crate::fig3d::bake_nodes(
                &st.figure,
                &st.pose,
                &st.camera,
                self.project.stage,
            );
            let root = baked.first().map(|b| b.xy).unwrap_or([0.0, 0.0]);
            (
                Self::fig3_bake_def(&st.figure, &baked),
                format!("{} (3D)", st.figure.name),
                st.figure.color,
                root,
            )
        });
        let Some((def, name, color, root)) = built else {
            self.status = "Enter 3D mode first.".into();
            return;
        };
        self.snapshot();
        let idx = self.project.figures.len();
        self.project.add_figure_with(def, &name, color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = format!(
            "Baked 3D pose into 2D figure {} ({name}), held on all frames.",
            idx + 1
        );
    }

    /// Mark the current 3D pose as "pose B": the target
    /// [`Self::fig3_bake_animation`] interpolates to from whatever
    /// pose is current when it runs. Pose the figure, mark B, pose
    /// it back, then bake the animation between the two.
    pub fn fig3_mark_pose_b(&mut self) {
        let msg = match &mut self.fig3 {
            Some(st) => {
                st.pose_b = Some(st.pose.clone());
                format!("3D pose B marked for {}.", st.figure.name)
            }
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// The directory holding the 3D figure library (`./library3d`,
    /// alongside the 2D `./library`).
    fn fig3_library_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library3d")
    }

    /// Save the session's 3D figure, in its current pose, to the
    /// 3D library so it survives the session.
    pub fn fig3_save_to_library(&mut self) {
        let msg = match &self.fig3 {
            Some(st) => match crate::fig3d::save_library_figure(
                &Self::fig3_library_dir(),
                &st.figure.name,
                &st.figure,
                &st.pose,
            ) {
                Ok(path) => {
                    format!("Saved 3D figure {} to {}.", st.figure.name, path.display())
                }
                Err(e) => format!("Could not save 3D figure: {e}"),
            },
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Load a 3D library figure by name, replacing the current
    /// session (or starting one). The camera is kept when a
    /// session is already active; pose B is cleared, as it
    /// belonged to the previous figure.
    pub fn fig3_load_from_library(&mut self, name: &str) {
        let path =
            Self::fig3_library_dir().join(crate::fig3d::library_file_name(name));
        let msg = match crate::fig3d::load_library_figure(&path) {
            Some((figure, pose)) => {
                let camera = self
                    .fig3
                    .as_ref()
                    .map(|st| st.camera.clone())
                    .unwrap_or_else(crate::fig3d::default_camera);
                let loaded_name = figure.name.clone();
                self.fig3 = Some(crate::fig3d::Fig3State {
                    figure,
                    pose,
                    camera,
                    selected: 0,
                    pose_b: None,
                });
                format!("Loaded 3D figure {loaded_name} from the library.")
            }
            None => format!("No 3D library figure named '{name}'."),
        };
        self.status = msg;
    }

    /// The display names of every figure in the 3D library,
    /// sorted (empty when the library directory is missing or
    /// holds no readable entries).
    pub fn fig3_library_list(&self) -> Vec<String> {
        crate::fig3d::scan_library(&Self::fig3_library_dir())
    }

    /// Load the next 3D library figure, cycling through
    /// [`Self::fig3_library_list`] the way [`Self::cycle_library`]
    /// walks the 2D library: with a session active, the entry
    /// after the current figure's name (wrapping); otherwise the
    /// first entry. An empty library only sets a status message.
    pub fn fig3_cycle_library(&mut self) {
        let names = self.fig3_library_list();
        if names.is_empty() {
            self.status = "3D library is empty — save a 3D figure first.".into();
            return;
        }
        let current = self.fig3.as_ref().map(|st| st.figure.name.clone());
        let next = match current {
            Some(name) => match names.iter().position(|n| *n == name) {
                Some(i) => names[(i + 1) % names.len()].clone(),
                None => names[0].clone(),
            },
            None => names[0].clone(),
        };
        self.fig3_load_from_library(&next);
    }

    /// Bake an ANIMATION: interpolate the session's current 3D
    /// pose to the marked pose B over `frames` frames and write
    /// the result as a new 2D figure whose frame poses follow the
    /// interpolation. The figure's definition is built from the
    /// first frame's projection (like [`Self::fig3_bake`]); each
    /// following frame's pose is that frame's projection
    /// recomputed against the definition (see
    /// [`Self::fig3_bake_pose`]). If the project has fewer than
    /// `frames` frames it is extended with held frames first.
    /// One snapshot covers the whole bake. Refuses — with a status
    /// message and no changes — without a session, without a
    /// marked pose B, or for `frames < 2`.
    pub fn fig3_bake_animation(&mut self, frames: usize) {
        if frames < 2 {
            self.status = "Bake animation needs at least 2 frames.".into();
            return;
        }
        let built = self.fig3.as_ref().and_then(|st| {
            let pose_b = st.pose_b.as_ref()?;
            let range = crate::fig3d::bake_range_staged(
                &st.figure,
                &st.pose,
                pose_b,
                &st.camera,
                self.project.stage,
                frames,
            );
            let poses: Vec<crate::model::Pose> = range
                .iter()
                .map(|b| Self::fig3_bake_pose(&st.figure, b))
                .collect();
            let root = range[0].first().map(|b| b.xy).unwrap_or([0.0, 0.0]);
            Some((
                Self::fig3_bake_def(&st.figure, &range[0]),
                format!("{} (3D anim)", st.figure.name),
                st.figure.color,
                root,
                poses,
            ))
        });
        let Some((def, name, color, root, poses)) = built else {
            self.status = if self.fig3.is_none() {
                "Enter 3D mode first.".into()
            } else {
                "Mark a pose B first, then bake the animation.".into()
            };
            return;
        };
        self.snapshot();
        let idx = self.project.figures.len();
        self.project.add_figure_with(def, &name, color, root);
        // Grow the timeline if the baked range is longer than the
        // project: appended frames hold the previous frame's poses
        // (the new figure included); the range overwrites the
        // first `frames` poses of the new figure below.
        while self.project.frames.len() < frames {
            let held = match self.project.frames.last() {
                Some(f) => f.poses.clone(),
                None => self
                    .project
                    .figures
                    .iter()
                    .map(|f| f.def.rest_pose([0.0, 0.0]))
                    .collect(),
            };
            self.project.frames.push(crate::model::Frame {
                poses: held,
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            });
        }
        for (f, pose) in poses.into_iter().enumerate() {
            self.project.frames[f].poses[idx] = pose;
        }
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = format!(
            "Baked 3D animation into 2D figure {} ({name}) over {frames} frames.",
            idx + 1
        );
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
    fn autosave_counter_counts_snapshots_only() {
        let mut core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        core.snapshot();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 3);
        // Ops that deliberately mutate without snapshotting
        // (non-undoable appearance cyclers) do not move the
        // counter: autosave counts snapshot-driven edits only.
        core.cycle_node_color();
        core.cycle_figure_opacity(0);
        assert_eq!(core.edits_since_autosave(), 3);
    }

    #[test]
    fn undo_and_redo_depth_track_the_stacks() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.redo_depth(), 0);
        core.add_frame();
        core.add_frame();
        assert_eq!(core.undo_depth(), 2);
        core.undo();
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.redo_depth(), 1);
        core.redo();
        assert_eq!(core.undo_depth(), 2);
        assert_eq!(core.redo_depth(), 0);
        // A fresh edit clears the redo stack.
        core.undo();
        assert_eq!(core.redo_depth(), 1);
        core.add_frame();
        assert_eq!(core.redo_depth(), 0);
    }

    #[test]
    fn node_style_setters_write_the_selected_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.set_node_cap(crate::model::CapKind::Square);
        assert_eq!(
            core.project.figures[0].def.nodes[1].style.cap,
            crate::model::CapKind::Square
        );
        // Same status wording as cycle_node_cap.
        assert_eq!(core.status, "Segment cap: Square");
        core.set_node_color2(Some(0x112233));
        assert_eq!(
            core.project.figures[0].def.nodes[1].style.color2,
            Some(0x112233)
        );
        assert_eq!(core.status, "Gradient to #112233.");
        core.set_node_color2(None);
        assert_eq!(core.project.figures[0].def.nodes[1].style.color2, None);
        assert_eq!(core.status, "Gradient off (solid colour).");
        core.set_node_opacity(0.5);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 0.5);
        // Opacity is clamped to 0.05..=1.0.
        core.set_node_opacity(2.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 1.0);
        core.set_node_opacity(0.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 0.05);
    }

    #[test]
    fn node_style_setters_no_op_without_a_selection() {
        let mut core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        let before = core.project.figures[0].def.nodes[1].style;
        core.set_node_cap(crate::model::CapKind::Point);
        assert_eq!(core.status, "Select a joint first.");
        core.set_node_color2(Some(0xFFFFFF));
        assert_eq!(core.status, "Select a joint first.");
        core.set_node_opacity(0.25);
        assert_eq!(core.status, "Select a joint first.");
        assert_eq!(core.project.figures[0].def.nodes[1].style, before);
    }

    #[test]
    fn set_project_easing_writes_without_snapshotting() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_project_easing(crate::model::Easing::EaseIn);
        assert_eq!(core.project.easing, crate::model::Easing::EaseIn);
        assert!(core.status.starts_with("Easing: "));
        // Mirrors cycle_easing: project-settings tweaks are
        // not undoable edits, so the undo stack is untouched.
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn delete_stroke_at_removes_the_indexed_stroke() {
        let mut core = EditorCore::new();
        let mk = |color| crate::model::Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color,
            width: 3.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
        };
        core.project.strokes = vec![mk(0x111111), mk(0x222222), mk(0x333333)];
        core.delete_stroke_at(1);
        let colors: Vec<u32> = core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colors, vec![0x111111, 0x333333]);
        assert_eq!(core.status, "Stroke 2 deleted.");
        // Out of range: status only, list untouched.
        core.delete_stroke_at(9);
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.status, "No stroke 10 to delete.");
    }

    #[test]
    fn autosave_due_respects_the_threshold() {
        let every = EditorCore::AUTOSAVE_EVERY;
        assert_eq!(every, 25);
        assert!(!EditorCore::autosave_due(0, 0));
        assert!(!EditorCore::autosave_due(every - 1, 0));
        assert!(EditorCore::autosave_due(every, 0));
        assert!(EditorCore::autosave_due(every * 2, every));
        assert!(!EditorCore::autosave_due(every * 2 - 1, every));
        // A last-autosave count ahead of the edit count (cannot
        // happen in practice) saturates instead of underflowing.
        assert!(!EditorCore::autosave_due(0, every));
    }

    #[test]
    fn maybe_autosave_noops_before_the_threshold() {
        let mut core = EditorCore::new();
        core.snapshot();
        let status_before = core.status.clone();
        core.maybe_autosave();
        // Below the threshold: no write, no status change (the
        // success path is silent by design), counter untouched.
        assert_eq!(core.edits_since_autosave(), 1);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn fig3_enter_exit_and_noop_bake() {
        let mut core = EditorCore::new();
        assert!(core.fig3.is_none());
        let figures_before = core.project.figures.len();
        core.fig3_bake();
        assert_eq!(
            core.project.figures.len(),
            figures_before,
            "bake without a 3D session is a no-op"
        );
        core.enter_3d();
        assert!(core.fig3.is_some());
        // Re-entering keeps the live session: a rotation survives it.
        core.fig3_rotate(2, 30.0);
        core.enter_3d();
        let st = core.fig3.as_ref().unwrap();
        assert_eq!(st.pose.rotations[st.selected][2], 30.0);
        core.exit_3d();
        assert!(core.fig3.is_none());
        core.exit_3d(); // a second exit is a harmless no-op
        assert!(core.fig3.is_none());
    }

    #[test]
    fn fig3_bake_joints_match_projection() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // Expected stage points straight from the fig3d pipeline.
        let expected = {
            let st = core.fig3.as_ref().unwrap();
            let world = crate::fig3d::evaluate3(&st.figure, &st.pose);
            crate::fig3d::project(&world, &st.camera, core.project.stage)
                .iter()
                .map(|p| p.xy)
                .collect::<Vec<_>>()
        };
        let figures_before = core.project.figures.len();
        core.fig3_bake();
        assert_eq!(core.project.figures.len(), figures_before + 1);
        let fi = core.project.figures.len() - 1;
        assert_eq!(core.selected_figure, fi);
        assert_eq!(core.project.figures[fi].def.nodes.len(), expected.len());
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), core.project.figures.len());
        }
        // The baked figure's evaluated joints equal the projected 3D
        // points in every frame (the bake is held as the rest pose).
        for f in 0..core.frame_count() {
            let joints = core.positions(f, fi);
            assert_eq!(joints.len(), expected.len());
            for (j, e) in joints.iter().zip(&expected) {
                assert!(
                    (j[0] - e[0]).abs() < 0.5 && (j[1] - e[1]).abs() < 0.5,
                    "frame {f}: joint {j:?} != projected {e:?}"
                );
            }
        }
        // Undo removes the baked figure again.
        core.undo();
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_rotate_and_orbit_change_the_bake() {
        let mut core = EditorCore::new();
        core.enter_3d();
        core.fig3_bake();
        let first = core.positions(0, core.project.figures.len() - 1);
        // Swing the left upper arm about Z: the left hand must move.
        core.fig3.as_mut().unwrap().selected = 3; // upperArm.L
        core.fig3_rotate(2, 90.0);
        core.fig3_bake();
        let second = core.positions(0, core.project.figures.len() - 1);
        let hand_l = 5;
        assert!(
            dist(first[hand_l], second[hand_l]) > 5.0,
            "hand.L should move: {:?} -> {:?}",
            first[hand_l],
            second[hand_l]
        );
        // Orbiting 180 degrees mirrors the figure on stage: the
        // right hand flips to the other side of the root.
        core.fig3_orbit(180.0, 0.0);
        assert_eq!(core.fig3.as_ref().unwrap().camera.yaw, 180.0);
        core.fig3_bake();
        let third = core.positions(0, core.project.figures.len() - 1);
        let root_x = third[0][0];
        let hand_r = 8;
        assert!(
            (second[hand_r][0] - root_x) * (third[hand_r][0] - root_x) < 0.0,
            "hand.R should mirror: {} vs {} around root {}",
            second[hand_r][0],
            third[hand_r][0],
            root_x
        );
    }

    #[test]
    fn fig3_bake_animation_requires_pose_b() {
        let mut core = EditorCore::new();
        let figures_before = core.project.figures.len();
        // No session at all.
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before);
        assert!(core.status.contains("3D mode"), "status: {}", core.status);
        // Session but no pose B marked.
        core.enter_3d();
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before);
        assert!(core.status.contains("pose B"), "status: {}", core.status);
        // Fewer than two frames is refused even with pose B marked.
        core.fig3_mark_pose_b();
        assert!(core.fig3.as_ref().unwrap().pose_b.is_some());
        core.fig3_bake_animation(1);
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_bake_animation_endpoints_match_projections() {
        let mut core = EditorCore::new();
        core.enter_3d();
        let project_frames = |core: &EditorCore| {
            let st = core.fig3.as_ref().unwrap();
            crate::fig3d::project(
                &crate::fig3d::evaluate3(&st.figure, &st.pose),
                &st.camera,
                core.project.stage,
            )
            .iter()
            .map(|p| p.xy)
            .collect::<Vec<_>>()
        };
        // Pose A = the rest pose the session starts in.
        let expected_a = project_frames(&core);
        // Pose B: swing the left upper arm 60 deg about Z (an
        // in-plane rotation, so projected segment lengths are
        // preserved and every frame can match its projection).
        core.fig3.as_mut().unwrap().selected = 3; // upperArm.L
        core.fig3_rotate(2, 60.0);
        let expected_b = project_frames(&core);
        core.fig3_mark_pose_b();
        // Pose back to A; the animation runs A -> B.
        core.fig3_rotate(2, -60.0);
        let figures_before = core.project.figures.len();
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert!(core.frame_count() >= 6, "timeline extended to the range");
        let fi = core.project.figures.len() - 1;
        assert_eq!(core.selected_figure, fi);
        for (f, expected) in [(0usize, &expected_a), (5usize, &expected_b)] {
            let joints = core.positions(f, fi);
            assert_eq!(joints.len(), expected.len());
            for (j, e) in joints.iter().zip(expected) {
                assert!(
                    (j[0] - e[0]).abs() < 0.5 && (j[1] - e[1]).abs() < 0.5,
                    "frame {f}: joint {j:?} != projected {e:?}"
                );
            }
        }
        // The middle of the range really moves: hand.L is well away
        // from its frame-0 spot by frame 3.
        let hand_l = 5;
        assert!(
            dist(core.positions(0, fi)[hand_l], core.positions(3, fi)[hand_l]) > 1.0,
            "hand.L should travel during the baked animation"
        );
        // One undo step removes the whole baked figure.
        core.undo();
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_library_save_and_load_via_editor() {
        let mut core = EditorCore::new();
        core.enter_3d();
        core.fig3_save_to_library();
        assert!(
            core.status.starts_with("Saved 3D figure"),
            "status: {}",
            core.status
        );
        // Loading a name that was never saved changes nothing.
        core.fig3_load_from_library("No Such Figure");
        assert!(core.status.contains("No 3D library figure"));
        assert!(core.fig3.is_some(), "session survives a failed load");
        // Round trip: drop the session, load the figure back.
        core.exit_3d();
        assert!(core.fig3.is_none());
        core.fig3_load_from_library("Humanoid3");
        let st = core.fig3.as_ref().expect("library load starts a session");
        assert_eq!(st.figure.name, "Humanoid3");
        assert_eq!(st.figure.nodes.len(), 15);
        assert!(st.pose_b.is_none());
        // Clean up the file this test wrote into ./library3d.
        let dir = EditorCore::fig3_library_dir();
        let _ = std::fs::remove_file(
            dir.join(crate::fig3d::library_file_name("Humanoid3")),
        );
        let _ = std::fs::remove_dir(&dir);
    }

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
    fn redo_restores_an_undone_frame_add() {
        let mut core = EditorCore::new();
        let n = core.frame_count();
        core.add_frame();
        assert_eq!(core.frame_count(), n + 1);
        core.undo();
        assert_eq!(core.frame_count(), n);
        core.redo();
        assert_eq!(core.frame_count(), n + 1);
    }

    #[test]
    fn figure_delete_and_reorder_keep_poses_aligned() {
        let mut core = EditorCore::new();
        core.add_figure();
        core.add_quadruped();
        assert_eq!(core.project.figures.len(), 3);
        core.select_figure(0);
        core.move_figure(1);
        assert_eq!(core.selected_figure, 1);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), 3);
        }
        core.delete_figure();
        assert_eq!(core.project.figures.len(), 2);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), 2);
        }
    }

    #[test]
    fn frame_clipboard_roundtrip() {
        let mut core = EditorCore::new();
        core.copy_frame();
        core.paste_frame();
        assert_eq!(core.frame_count(), 6);
    }

    #[test]
    fn drawing_strokes_attach_to_the_frame() {
        let mut core = EditorCore::new();
        core.begin_draw([100.0, 100.0]);
        core.draw_to([140.0, 120.0]);
        core.draw_to([180.0, 110.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].points.len(), 3);
        core.clear_frame_drawings();
        assert!(core.project.strokes.is_empty());
    }

    #[test]
    fn pressure_widths_follow_pointer_speed() {
        let mut core = EditorCore::new();
        core.begin_draw([0.0, 0.0]);
        core.draw_to([3.0, 0.0]); // slow 3px step
        core.draw_to([120.0, 0.0]); // fast flick
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel to points");
        assert!((s.width - 4.0).abs() < f32::EPSILON, "base width unchanged");
        for w in &s.widths {
            assert!(
                *w >= 4.0 * 0.45 - 0.01 && *w <= 4.0 * 1.8 + 0.01,
                "width {w} within the pressure range"
            );
        }
        assert!(
            s.widths[1] > s.widths[2],
            "slow point wider than the fast flick: {:?}",
            s.widths
        );
    }

    fn set_brush(core: &mut EditorCore, name: &str) {
        core.brush_index = crate::paint::brush_library()
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("no brush named {name}"));
    }

    #[test]
    fn default_pen_brush_leaves_the_recorded_stroke_untouched() {
        // The Pen preset (index 0, the default) must reproduce the
        // pre-library behaviour exactly: no smoothing, no nib
        // profile, base width 4.0.
        let mut core = EditorCore::new();
        assert_eq!(core.brush_index, 0);
        assert_eq!(core.active_brush().name, "Pen");
        core.begin_draw([0.0, 0.0]);
        core.draw_to([10.0, 5.0]);
        core.draw_to([25.0, 5.0]);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.points, vec![[0.0, 0.0], [10.0, 5.0], [25.0, 5.0]]);
        assert_eq!(s.width, 4.0);
    }

    #[test]
    fn smoothing_brush_adds_points_at_stroke_end() {
        let mut core = EditorCore::new();
        set_brush(&mut core, "Marker"); // width 8, smoothing 1
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.draw_to([60.0, 30.0]);
        // Before the stroke ends, only the 3 recorded points exist.
        assert_eq!(core.project.strokes[0].points.len(), 3);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.points.len(), 6, "3 recorded points, Chaikin x2: {:?}", s.points);
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel");
        assert_eq!(s.width, 8.0, "the brush sets the stroke's base width");
        // Smoothing pins the endpoints.
        assert_eq!(s.points.first(), Some(&[0.0, 0.0]));
        assert_eq!(s.points.last(), Some(&[60.0, 30.0]));
    }

    #[test]
    fn calligraphy_brush_profiles_widths_at_stroke_end() {
        let mut core = EditorCore::new();
        set_brush(&mut core, "Calligraphy"); // width 7, nib 30 degrees
        core.begin_draw([0.0, 0.0]);
        core.draw_to([40.0, 0.0]);
        core.draw_to([40.0, 40.0]);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.widths.len(), s.points.len(), "profile is parallel");
        let max = s.widths.iter().copied().fold(f32::MIN, f32::max);
        let min = s.widths.iter().copied().fold(f32::MAX, f32::min);
        assert!(max > min, "widths vary with travel direction: {:?}", s.widths);
        assert!(
            s.widths.iter().all(|&w| w >= 0.15 * 7.0 - 0.01 && w <= 7.0 + 0.01),
            "widths within the nib's range: {:?}",
            s.widths
        );
    }

    #[test]
    fn eraser_tool_splits_strokes_on_the_current_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect(),
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
        });
        core.toggle_draw_tool();
        assert_eq!(core.draw_tool, DrawTool::Eraser);
        // Pen width 4 -> eraser radius 8: the press at x=50 removes
        // only that point and splits the stroke; dragging to x=90
        // removes that point too (x=100 is left a singleton and
        // vanishes with it).
        core.begin_draw([50.0, 0.0]);
        assert_eq!(core.project.strokes.len(), 2, "split by the press");
        core.draw_to([90.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[0].points.last(), Some(&[40.0, 0.0]));
        assert_eq!(
            core.project.strokes[1].points,
            vec![[60.0, 0.0], [70.0, 0.0], [80.0, 0.0]]
        );
        assert!(
            core.status.contains("Eraser: 2 stroke(s) changed"),
            "status reports both erase steps: {}",
            core.status
        );
        // Toggling back restores the brush tool.
        core.toggle_draw_tool();
        assert_eq!(core.draw_tool, DrawTool::Brush);
    }

    #[test]
    fn cycle_brush_walks_the_library_and_wraps() {
        let mut core = EditorCore::new();
        let n = crate::paint::brush_library().len();
        assert_eq!(core.active_brush().name, "Pen");
        for _ in 0..n {
            core.cycle_brush();
        }
        assert_eq!(core.brush_index, 0, "a full cycle wraps to the start");
        assert_eq!(core.active_brush().name, "Pen");
        core.cycle_brush();
        assert_eq!(core.active_brush().name, "Pencil");
        assert!(core.status.contains("Pencil"), "status names the brush: {}", core.status);
    }

    #[test]
    fn fill_appends_scanline_stroke_and_marks_outline() {
        let mut core = EditorCore::new();
        core.begin_draw([100.0, 100.0]);
        core.draw_to([200.0, 100.0]);
        core.draw_to([200.0, 200.0]);
        core.draw_to([100.0, 200.0]);
        core.end_draw();
        let color = core.project.strokes[0].color;
        core.fill_current_stroke();
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[0].fill, Some(color));
        let fill = &core.project.strokes[1];
        assert_eq!(fill.color, color);
        assert_eq!(fill.frame, 0);
        assert!(fill.points.len() > 8, "fill has many scan points: {}", fill.points.len());
        for p in &fill.points {
            assert!(
                p[0] >= 99.9 && p[0] <= 200.1 && p[1] >= 99.9 && p[1] <= 200.1,
                "fill point within the shape bounds: {p:?}"
            );
        }
        core.undo();
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].fill, None);
    }

    #[test]
    fn fill_without_a_shape_is_a_no_op() {
        let mut core = EditorCore::new();
        core.fill_current_stroke();
        assert!(core.project.strokes.is_empty());
        core.begin_draw([10.0, 10.0]);
        core.draw_to([60.0, 10.0]);
        core.end_draw();
        core.fill_current_stroke(); // only 2 points: not a shape
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].fill, None);
    }

    #[test]
    fn lasso_erases_only_mostly_inside_strokes() {
        let mut core = EditorCore::new();
        // Stroke fully inside the lasso box.
        core.begin_draw([10.0, 10.0]);
        core.draw_to([20.0, 20.0]);
        core.draw_to([30.0, 10.0]);
        core.end_draw();
        // Stroke fully outside.
        core.begin_draw([400.0, 400.0]);
        core.draw_to([430.0, 420.0]);
        core.draw_to([460.0, 400.0]);
        core.end_draw();
        // Straddling stroke: 1 of 3 points inside -> survives.
        core.begin_draw([20.0, 20.0]);
        core.draw_to([400.0, 20.0]);
        core.draw_to([20.0, 400.0]);
        core.end_draw();
        let lasso = vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [0.0, 50.0]];
        core.lasso_erase(&lasso);
        assert_eq!(core.project.strokes.len(), 2);
        core.undo();
        assert_eq!(core.project.strokes.len(), 3);
        // A degenerate lasso erases nothing.
        core.lasso_erase(&[[0.0, 0.0], [10.0, 10.0]]);
        assert_eq!(core.project.strokes.len(), 3);
    }

    #[test]
    fn movieclip_loops_independently_of_the_timeline() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        assert_eq!(core.project.clips.len(), 1);
        assert_eq!(core.project.figures[0].clip, Some(0));
        // Corrupt the timeline pose; the clip pose must not follow.
        core.project.frames[0].poses[0].root = [0.0, 0.0];
        let poses = core.project.poses_at(0.0);
        assert!(poses[0].root[0] > 400.0, "clip kept its own root, got {:?}", poses[0].root);
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

    #[test]
    fn wave7_cycle_bend_steps_and_storage() {
        let mut c = EditorCore::new();
        let nodes = c.project.figures[0].def.nodes.len();
        // No joint selected: explains itself, changes nothing.
        c.cycle_bend();
        assert!(c.project.frames[0].bends.is_empty());
        assert!(c.status.contains("Select a joint"), "status: {}", c.status);
        // The origin has no segment to bend.
        c.selected_node = Some(0);
        c.cycle_bend();
        assert!(c.project.frames[0].bends.is_empty());
        assert!(c.status.contains("origin"), "status: {}", c.status);
        // First step lands and is visible through bends_at.
        c.selected_node = Some(2);
        c.cycle_bend();
        {
            let b = &c.project.frames[0].bends;
            assert_eq!(b.len(), 1, "one figure entry");
            assert_eq!(b[0].len(), nodes, "per-node storage shape");
            assert!((b[0][2] - 0.35).abs() < 1e-6);
        }
        assert!((c.project.bends_at(0.0, 0)[2] - 0.35).abs() < 1e-6);
        // The rest of the cycle: +0.7, -0.35, -0.7, back to 0.
        for w in [0.7f32, -0.35, -0.7, 0.0] {
            c.cycle_bend();
            let got = c.project.frames[0].bends[0][2];
            assert!((got - w).abs() < 1e-6, "bend {got} vs {w}");
        }
        // All-zero again: bends_at reports "no bends" (empty).
        assert!(c.project.bends_at(0.0, 0).is_empty());
    }

    #[test]
    fn wave5_locked_figure_refuses_drag() {
        let mut c = EditorCore::new();
        let pos = c.positions(0, 0);
        let hand = pos[pos.len() - 1];
        c.project.figures[0].locked = true;
        assert!(!c.begin_drag(hand), "locked figure must refuse drags");
        c.toggle_figure_lock(0);
        assert!(c.begin_drag(hand), "unlocked figure drags again");
        c.end_drag();
    }

    #[test]
    fn wave5_graph_key_drives_positions() {
        let mut c = EditorCore::new();
        c.selected_node = Some(2);
        c.key_graph_angle();
        assert_eq!(c.project.channels.len(), 1);
        let base = c.positions_f(0.0, 0);
        // Rewrite the channel to a very different angle at frame 0 and
        // confirm positions_f (the editor's sampling path) follows it.
        c.project.channels[0].keys[0].value += 120.0;
        // Hermite with a single key holds its value everywhere.
        let moved = c.positions_f(0.0, 0);
        assert!(
            (moved[2][0] - base[2][0]).abs() + (moved[2][1] - base[2][1]).abs() > 1.0,
            "graph channel must change the sampled pose"
        );
        c.clear_graph();
        assert!(c.project.channels.is_empty());
    }

    #[test]
    fn wave5_move_figure_full_remaps_selection_and_solo() {
        let mut c = EditorCore::new();
        c.add_figure(); // two figures
        assert_eq!(c.project.figures.len(), 2);
        c.selected_figure = 0;
        c.solo = Some(0);
        c.move_figure_full(1);
        assert_eq!(c.selected_figure, 1);
        assert_eq!(c.solo, Some(1));
    }

    fn audio_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-editor-audio-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_editor_wav(
        dir: &PathBuf,
        name: &str,
        data: &crate::audio_edit::WavData,
    ) -> String {
        let path = dir.join(name);
        std::fs::write(&path, crate::audio_edit::write_wav(data)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn const_editor_wav(
        rate: u32,
        frames: usize,
        value: f32,
    ) -> crate::audio_edit::WavData {
        crate::audio_edit::WavData {
            sample_rate: rate,
            channels: 1,
            samples: vec![value; frames],
        }
    }

    fn audio_editor_core(fps: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.audio.clear();
        core
    }

    fn assert_audio_clip(
        core: &EditorCore,
        idx: usize,
        path: &str,
        start_frame: usize,
    ) {
        assert_eq!(core.project.audio[idx].path, path);
        assert_eq!(core.project.audio[idx].start_frame, start_frame);
    }

    #[test]
    fn wave6_audio_normalize_writes_norm_file_and_repoints_clip() {
        let dir = audio_temp_dir("normalize");
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.25, -0.125, 0.05],
        };
        let source = write_editor_wav(&dir, "voice.wav", &source_data);
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 3,
            gain: 1.0,
        });

        core.audio_normalize(0);

        let expected = dir.join("voice.norm.wav");
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 3);
        assert!(expected.exists(), "normalised file should be written");
        let out = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert!(
            (crate::audio_edit::peak(&out) - 0.9).abs() < 0.001,
            "normalised peak {}",
            crate::audio_edit::peak(&out)
        );
        let original = crate::audio_edit::read_wav(&source).unwrap();
        assert!((crate::audio_edit::peak(&original) - 0.25).abs() < 0.001);
        assert!(core.status.contains("gain 3.60x"), "status: {}", core.status);

        // A bad index changes neither the clip list nor its entries.
        core.audio_normalize(99);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 3);
        assert!(core.status.contains("failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_normalize_rejects_non_wav_without_changes() {
        let dir = audio_temp_dir("normalize-bad");
        let bad_path = dir.join("bad.wav");
        std::fs::write(&bad_path, b"this is not a wav").unwrap();
        let source = bad_path.to_string_lossy().into_owned();
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 4,
            gain: 1.0,
        });

        core.audio_normalize(0);

        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 4);
        assert!(!dir.join("bad.norm.wav").exists());
        assert!(core.status.contains("Audio normalise failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_trim_writes_trim_file_and_shifts_start_frame() {
        let dir = audio_temp_dir("trim");
        let samples: Vec<f32> = (0..8000)
            .map(|i| {
                if i < 2000 {
                    0.1
                } else if i < 4000 {
                    0.2
                } else {
                    0.3
                }
            })
            .collect();
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let source = write_editor_wav(&dir, "clip.wav", &source_data);
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source,
            start_frame: 7,
            gain: 1.0,
        });

        core.audio_trim(0, 0.25, 0.75);

        let expected = dir.join("clip.trim.wav");
        // At 12 fps, trimming a 0.25s head shifts frame 7 by 3 to 10.
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 10);
        let trimmed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(trimmed.frames(), 4000);
        assert!((trimmed.samples[0] - 0.2).abs() < 0.001);
        assert!((trimmed.samples[3999] - 0.3).abs() < 0.001);
    }

    #[test]
    fn wave6_audio_trim_rejects_empty_range_without_changes() {
        let dir = audio_temp_dir("trim-empty");
        let source = write_editor_wav(&dir, "range.wav", &const_editor_wav(8000, 8000, 0.25));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 9,
            gain: 1.0,
        });

        core.audio_trim(0, 0.75, 0.25);

        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 9);
        assert!(!dir.join("range.trim.wav").exists());
        assert!(core.status.contains("Audio trim failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_split_materialises_both_halves_and_inserts_second_clip() {
        let dir = audio_temp_dir("split");
        let mut samples = vec![0.25; 4000];
        samples.extend(vec![-0.5; 4000]);
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let source = write_editor_wav(&dir, "take.wav", &source_data);
        let other = write_editor_wav(&dir, "other.wav", &const_editor_wav(8000, 800, 0.1));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source,
            start_frame: 12,
            gain: 1.0,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: other.clone(),
            start_frame: 30,
            gain: 1.0,
        });

        // The source lasts 12 timeline frames (1s at 12 fps), so
        // frame 18 is its midpoint: 0.5s / 4000 sample frames in.
        core.audio_split(0, 18);

        let first_path = dir.join("take.a.wav");
        let second_path = dir.join("take.b.wav");
        assert_eq!(core.project.audio.len(), 3);
        assert_audio_clip(&core, 0, &first_path.to_string_lossy(), 12);
        assert_audio_clip(&core, 1, &second_path.to_string_lossy(), 18);
        assert_audio_clip(&core, 2, &other, 30);
        let first = crate::audio_edit::read_wav(&first_path.to_string_lossy()).unwrap();
        let second = crate::audio_edit::read_wav(&second_path.to_string_lossy()).unwrap();
        assert_eq!(first.frames() + second.frames(), source_data.frames());
        assert_eq!(first.frames(), 4000);
        assert_eq!(second.frames(), 4000);
        assert!(first.samples.iter().all(|s| (*s - 0.25).abs() < 0.001));
        assert!(second.samples.iter().all(|s| (*s + 0.5).abs() < 0.001));
    }

    #[test]
    fn wave6_audio_split_rejects_boundaries_without_changes() {
        let dir = audio_temp_dir("split-boundary");
        let source = write_editor_wav(&dir, "edge.wav", &const_editor_wav(8000, 8000, 0.25));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 12,
            gain: 1.0,
        });

        // The clip spans timeline frames 12..24; neither endpoint is
        // strictly inside it.
        core.audio_split(0, 12);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 12);
        core.audio_split(0, 24);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 12);
        assert!(!dir.join("edge.a.wav").exists());
        assert!(!dir.join("edge.b.wav").exists());
        assert!(core.status.contains("Audio split failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_crossfade_uses_overlap_and_replaces_pair() {
        let dir = audio_temp_dir("crossfade-overlap");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 4000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first,
            start_frame: 0,
            gain: 1.0,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 3,
            gain: 1.0,
        });

        // Each clip lasts 0.5s. The second starts at 0.3s, so the
        // overlap (and therefore the fade) is 0.2s / 1600 frames.
        core.audio_crossfade(0);

        let expected = dir.join("a.xfade.wav");
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 0);
        let mixed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(mixed.frames(), 4000 + 4000 - 1600);
        assert!((mixed.samples[0] - 0.4).abs() < 0.001);
        assert!(
            (mixed.samples[3200] - 0.6).abs() < 0.001,
            "fade midpoint {}",
            mixed.samples[3200]
        );
        assert!((mixed.samples[4000] - 0.8).abs() < 0.001);
        assert!(core.status.contains("0.20s"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_crossfade_adjacent_pair_uses_one_second_fade() {
        let dir = audio_temp_dir("crossfade-adjacent");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 12000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 12000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first,
            start_frame: 0,
            gain: 1.0,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 15,
            gain: 1.0,
        });

        // Two adjacent 1.5s clips: the default fade is 1.0s, so
        // the joined file lasts 1.5 + 1.5 - 1.0 = 2.0s.
        core.audio_crossfade(0);

        let expected = dir.join("a.xfade.wav");
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 0);
        let mixed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(mixed.frames(), 16000);
        assert!((mixed.samples[0] - 0.4).abs() < 0.001);
        assert!((mixed.samples[8000] - 0.6).abs() < 0.001);
        assert!((mixed.samples[15999] - 0.8).abs() < 0.001);
    }

    #[test]
    fn wave6_audio_crossfade_rejects_gap_without_changes() {
        let dir = audio_temp_dir("crossfade-gap");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 4000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first.clone(),
            start_frame: 0,
            gain: 1.0,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 10,
            gain: 1.0,
        });

        // The first 0.5s clip ends at frame 5; the second starts at
        // frame 10, leaving a 0.5s gap.
        core.audio_crossfade(0);

        assert_eq!(core.project.audio.len(), 2);
        assert_audio_clip(&core, 0, &first, 0);
        assert_audio_clip(&core, 1, &second, 10);
        assert!(!dir.join("a.xfade.wav").exists());
        assert!(core.status.contains("Audio crossfade failed"), "status: {}", core.status);
    }

    // ---- Wave 7: timeline depth -------------------------------------

    /// Tag every demo frame with a unique root x, duration and
    /// bend so frame order is directly observable.
    fn tag_frames(core: &mut EditorCore) {
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [i as f32 * 10.0, 100.0];
            frame.duration = (i + 1) as f32;
            frame.bends = vec![vec![i as f32]];
        }
        core.project.frames[2].easing = Some(crate::model::Easing::EaseIn);
    }

    fn root_xs(core: &EditorCore) -> Vec<f32> {
        core.project
            .frames
            .iter()
            .map(|f| f.poses[0].root[0])
            .collect()
    }

    #[test]
    fn wave7_loop_range_orders_and_clamps() {
        let mut core = EditorCore::new(); // 5 demo frames
        core.set_loop_range(3, 1);
        assert_eq!(core.loop_range, Some((1, 3)), "endpoints are ordered");
        core.set_loop_range(0, 99);
        assert_eq!(core.loop_range, Some((0, 4)), "clamped to the timeline");
        core.set_loop_range(2, 2);
        assert_eq!(core.loop_range, Some((2, 2)), "a single-frame loop is valid");
        core.clear_loop_range();
        assert_eq!(core.loop_range, None);
    }

    #[test]
    fn wave7_loop_range_wraps_playback_at_its_end() {
        let mut core = EditorCore::new(); // fps 12, all durations 1.0
        core.set_loop_range(1, 3);
        // One authored frame of playback per 1/12 s.
        let step = 1.0 / core.project.fps;
        core.goto(1);
        core.advance_play(step);
        assert_eq!(core.current, 2, "inside the range: no wrap");
        core.goto(3);
        core.advance_play(step);
        assert_eq!(core.current, 1, "past frame 3 the playhead wraps to the range start");
        assert!((core.playhead - 1.0).abs() < 1e-4, "playhead: {}", core.playhead);
        // Fractional overshoot survives the wrap.
        core.goto(3);
        core.advance_play(1.5 * step);
        assert_eq!(core.current, 1);
        assert!((core.playhead - 1.5).abs() < 1e-4, "playhead: {}", core.playhead);
        // A single-frame loop pins playback to that frame.
        core.set_loop_range(2, 2);
        core.goto(2);
        core.advance_play(step);
        assert_eq!(core.current, 2);
        core.advance_play(step);
        assert_eq!(core.current, 2);
    }

    #[test]
    fn wave7_loop_range_survives_goto_outside_it() {
        let mut core = EditorCore::new();
        core.set_loop_range(2, 3);
        core.goto(0); // before the range
        assert_eq!(core.loop_range, Some((2, 3)), "goto leaves the range intact");
        // Playing forward from before the range enters it, then wraps.
        let step = 1.0 / core.project.fps;
        core.advance_play(step);
        assert_eq!(core.current, 1);
        core.advance_play(step);
        assert_eq!(core.current, 2);
        core.advance_play(step);
        assert_eq!(core.current, 3);
        core.advance_play(step);
        assert_eq!(core.current, 2, "wrapped at the range end");
        assert_eq!(core.loop_range, Some((2, 3)), "range intact after playback");
    }

    #[test]
    fn wave7_no_loop_range_playback_unchanged() {
        let mut core = EditorCore::new();
        assert_eq!(core.loop_range, None);
        let step = 1.0 / core.project.fps;
        core.goto(4);
        core.advance_play(step);
        assert_eq!(core.current, 0, "wraps around the whole timeline, as before");
        assert!(core.playhead.abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_play(step);
        assert_eq!(core.current, 1);
    }

    #[test]
    fn wave7_shift_frames_moves_block_with_all_frame_data() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.goto(0);
        assert!(core.shift_frames(0, 2, 2), "move frames 1-2 later by 2");
        assert_eq!(root_xs(&core), vec![20.0, 30.0, 0.0, 10.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![3.0, 4.0, 1.0, 2.0, 5.0], "durations travel");
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![2.0, 3.0, 0.0, 1.0, 4.0], "bends travel");
        assert_eq!(
            core.project.frames[0].easing,
            Some(crate::model::Easing::EaseIn),
            "easing travels (tagged frame 2 is now first)"
        );
        assert_eq!(core.current, 2, "current follows its frame");
        // Undo restores the original order.
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave7_shift_frames_earlier_and_refusals() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        assert!(core.shift_frames(3, 2, -2), "move frames 4-5 earlier by 2");
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 40.0, 10.0, 20.0]);
        // Refusals change nothing.
        let before = root_xs(&core);
        assert!(!core.shift_frames(3, 2, 1), "destination past the end");
        assert!(!core.shift_frames(0, 2, -1), "destination before the start");
        assert!(!core.shift_frames(4, 2, -1), "block past the end");
        assert!(!core.shift_frames(0, 0, 1), "empty block");
        assert!(!core.shift_frames(0, 2, 0), "zero delta");
        assert_eq!(root_xs(&core), before, "refused shifts are no-ops");
    }

    #[test]
    fn wave7_reverse_range_reverses_frames() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.goto(1);
        core.reverse_range(1, 3);
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 20.0, 10.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 4.0, 3.0, 2.0, 5.0], "durations reverse too");
        assert_eq!(core.current, 3, "current flips inside the reversed block");
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave9_delete_range_removes_middle_and_remaps_state() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.set_loop_range(0, 4);
        core.goto(2); // inside the block being deleted
        assert!(core.delete_range(1, 2));
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 4.0, 5.0], "surviving durations keep their frames");
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![0.0, 3.0, 4.0], "surviving bends keep their frames");
        assert_eq!(core.current, 1, "current lands on the next surviving frame");
        assert_eq!(core.project.frames[core.current].poses[0].root[0], 30.0);
        assert_eq!(
            core.loop_range,
            Some((0, 2)),
            "loop range follows its surviving first and last frames"
        );
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave9_delete_range_refuses_every_frame_and_clamps_tail() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        let before = root_xs(&core);
        assert!(!core.delete_range(0, 99), "deleting every frame is refused");
        assert!(!core.delete_range(5, 1), "start past the end is refused");
        assert!(!core.delete_range(0, 0), "an empty range is refused");
        assert_eq!(root_xs(&core), before, "refused deletions are no-ops");

        core.set_loop_range(3, 4);
        core.goto(4);
        assert!(core.delete_range(3, 99), "count clamps to the timeline end");
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0]);
        assert_eq!(core.current, 2, "current lands on the last surviving frame");
        assert_eq!(core.loop_range, None, "a loop range inside the deleted tail is cleared");
    }

    #[test]
    fn wave9_copy_paste_range_round_trip_preserves_poses() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        assert!(core.copy_range(1, 2));
        assert_eq!(core.frame_clipboard.len(), 2);
        assert!(core.clipboard.is_none(), "the single-frame clipboard is separate");
        core.project.frames[1].poses[0].root[0] = 999.0;
        assert_eq!(
            core.frame_clipboard[0].poses[0].root[0], 10.0,
            "the range clipboard holds deep clones"
        );
        assert!(!core.copy_range(5, 1), "copy start past the end is refused");
        assert!(!core.copy_range(0, 0), "an empty copy is refused");
        assert_eq!(core.frame_clipboard.len(), 2, "refused copies keep the clipboard");

        core.set_loop_range(3, 4);
        core.goto(4);
        assert!(core.paste_range(4));
        assert_eq!(root_xs(&core), vec![0.0, 999.0, 20.0, 30.0, 10.0, 20.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 5.0]);
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![0.0, 1.0, 2.0, 3.0, 1.0, 2.0, 4.0]);
        assert_eq!(
            core.project.frames[5].easing,
            Some(crate::model::Easing::EaseIn),
            "the copied frame's easing is preserved"
        );
        assert_eq!(core.current, 6, "current follows its original frame past the insertion");
        assert_eq!(core.loop_range, Some((3, 6)), "loop endpoints follow their frames");
        assert_eq!(core.frame_clipboard.len(), 2, "paste does not consume the clipboard");
    }

    #[test]
    fn wave9_paste_range_at_end_appends_and_empty_clipboard_refuses() {
        let mut core = EditorCore::new();
        assert!(!core.paste_range(0), "an empty range clipboard is refused");
        assert_eq!(core.project.frames.len(), 5);
        tag_frames(&mut core);
        assert!(core.copy_range(3, 2));
        core.goto(0);
        assert!(core.paste_range(999), "the paste index clamps to the end");
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0, 30.0, 40.0]);
        assert_eq!(core.current, 0, "appending does not move an earlier current frame");
    }

    #[test]
    fn wave7_nudge_path_point_moves_only_the_target_root() {
        let mut core = EditorCore::new();
        let angles_before = core.project.frames[2].poses[0].angles.clone();
        let roots_before = root_xs(&core);
        core.nudge_path_point(2, 0, [5.0, -3.0]);
        let root = core.project.frames[2].poses[0].root;
        assert_eq!(root, [roots_before[2] + 5.0, 470.0 - 3.0]);
        assert_eq!(core.project.frames[1].poses[0].root[0], roots_before[1]);
        assert_eq!(core.project.frames[3].poses[0].root[0], roots_before[3]);
        assert_eq!(
            core.project.frames[2].poses[0].angles, angles_before,
            "angles untouched"
        );
        // Out-of-range targets are reported, not panics.
        core.nudge_path_point(99, 0, [1.0, 1.0]);
        assert!(core.status.contains("No such frame"), "status: {}", core.status);
        assert_eq!(root_xs(&core)[2], roots_before[2] + 5.0);
    }

    #[test]
    fn wave7_smooth_path_roots_averages_a_spike() {
        let mut core = EditorCore::new();
        // Roots on x: still, still, SPIKE, still, still. y constant.
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [if i == 2 { 100.0 } else { 0.0 }, 50.0];
        }
        let angles_before = core.project.frames[2].poses[0].angles.clone();
        core.smooth_path_roots(0);
        assert_eq!(
            root_xs(&core),
            vec![0.0, 50.0, 0.0, 50.0, 0.0],
            "one pass over the original values: the spike collapses, \
             its neighbours take the midpoint"
        );
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].root[1], 50.0, "y untouched");
        }
        assert_eq!(
            core.project.frames[2].poses[0].angles, angles_before,
            "angles untouched"
        );
        assert_eq!(core.project.frames[0].poses[0].root, [0.0, 50.0], "first endpoint kept");
        assert_eq!(core.project.frames[4].poses[0].root, [0.0, 50.0], "last endpoint kept");
    }

    #[test]
    fn wave6_audio_crossfade_rejects_unreadable_pair_without_changes() {
        let dir = audio_temp_dir("crossfade-bad");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let bad_path = dir.join("b.wav");
        std::fs::write(&bad_path, b"not a wav").unwrap();
        let second = bad_path.to_string_lossy().into_owned();
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first.clone(),
            start_frame: 0,
            gain: 1.0,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 3,
            gain: 1.0,
        });

        core.audio_crossfade(0);

        assert_eq!(core.project.audio.len(), 2);
        assert_audio_clip(&core, 0, &first, 0);
        assert_audio_clip(&core, 1, &second, 3);
        assert!(!dir.join("a.xfade.wav").exists());
        assert!(core.status.contains("Audio crossfade failed"), "status: {}", core.status);
    }

    #[test]
    fn wave7_clip_create_from_figure_round_trip() {
        let mut core = EditorCore::new();
        core.project.tween_playback = true;
        let times = [0.0f32, 1.0, 1.5, 2.0, 3.0, 4.0];
        let before: Vec<Vec<V2>> = times.iter().map(|&f| core.positions_f(f, 0)).collect();
        let ci = core.create_clip_from_figure(0).expect("clip created");
        assert_eq!(core.project.figures[0].clip, Some(ci));
        assert_eq!(core.project.clips[ci].frames.len(), core.project.frames.len());
        assert_eq!(
            core.project.clips[ci].def.nodes.len(),
            core.project.figures[0].def.nodes.len()
        );
        for (k, &f) in times.iter().enumerate() {
            assert_eq!(
                core.positions_f(f, 0),
                before[k],
                "frame {f} changed after clip conversion"
            );
        }
        // The conversion snapshot: undo detaches the instance again.
        core.undo();
        assert_eq!(core.project.figures[0].clip, None);
    }

    #[test]
    fn wave7_clip_session_edits_only_the_clip() {
        let mut core = EditorCore::new();
        let ci = core.create_clip_from_figure(0).unwrap();
        let timeline_before = serde_json::to_string(&core.project.frames).unwrap();
        assert!(core.enter_clip(ci));
        assert_eq!(core.clip_edit, Some(ClipEditSession { clip: ci, frame: 0 }));
        core.clip_goto(1);
        core.clip_set_angle(3, 42.0);
        core.clip_set_root([10.0, 20.0]);
        assert_eq!(core.project.clips[ci].frames[1].angles[3], 42.0);
        assert_eq!(core.project.clips[ci].frames[1].root, [10.0, 20.0]);
        // Positions evaluate the clip's own def and session pose.
        let pos = core.clip_positions();
        assert_eq!(pos.len(), core.project.clips[ci].def.nodes.len());
        assert_eq!(pos[0], [10.0, 20.0]);
        // Add duplicates the last pose and moves the session onto
        // it; delete removes the session frame again.
        let n = core.project.clips[ci].frames.len();
        let last = core.project.clips[ci].frames[n - 1].clone();
        core.clip_add_frame();
        assert_eq!(core.project.clips[ci].frames.len(), n + 1);
        assert_eq!(core.project.clips[ci].frames[n].root, last.root);
        assert_eq!(core.project.clips[ci].frames[n].angles, last.angles);
        assert_eq!(core.clip_edit.unwrap().frame, n);
        core.clip_delete_frame();
        assert_eq!(core.project.clips[ci].frames.len(), n);
        // The project timeline was never touched.
        assert_eq!(
            serde_json::to_string(&core.project.frames).unwrap(),
            timeline_before
        );
        core.exit_clip();
        assert!(core.clip_edit.is_none());
    }

    #[test]
    fn wave7_clip_session_boundaries() {
        let mut core = EditorCore::new();
        // No session: every session op is a no-op.
        core.clip_add_frame();
        core.clip_delete_frame();
        core.clip_goto(2);
        core.clip_set_angle(0, 10.0);
        core.clip_set_root([0.0, 0.0]);
        assert!(core.project.clips.is_empty());
        assert!(core.clip_positions().is_empty());
        assert!(core.status.contains("Not editing"));
        // Invalid entry points.
        assert!(!core.enter_clip(0));
        assert!(core.create_clip_from_figure(99).is_none());
        // A one-frame clip refuses deletion of its only frame.
        let ci = core.create_clip_from_figure(0).unwrap();
        core.project.clips[ci].frames.truncate(1);
        assert!(core.enter_clip(ci));
        core.clip_delete_frame();
        assert_eq!(core.project.clips[ci].frames.len(), 1);
        assert!(core.status.contains("at least one frame"));
        // Out-of-range joint is refused without changes.
        let angles_before = core.project.clips[ci].frames[0].angles.clone();
        core.clip_set_angle(999, 5.0);
        assert_eq!(core.project.clips[ci].frames[0].angles, angles_before);
        // Goto clamps to the final frame (frame 0 of one).
        core.clip_goto(500);
        assert_eq!(core.clip_edit.unwrap().frame, 0);
        core.exit_clip();
        core.exit_clip(); // a second exit is a harmless no-op
        assert!(core.clip_edit.is_none());
    }
}

#[cfg(test)]
mod wave10_tests {
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::{ClipDef, ClipLoop, FigureDef};

    #[test]
    fn smooth_graph_tangents_rewrites_flat_tangents() {
        let mut core = EditorCore::new();
        // The demo project has no channels: a clear no-op status,
        // and no undo step is recorded for it.
        core.smooth_graph_tangents();
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );

        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 10.0),
            ],
        });
        core.smooth_graph_tangents();
        assert!(core.status.contains("Smoothed"), "status: {}", core.status);
        let ch = &core.project.channels[0];
        // Frames and values are untouched; only tangents move.
        assert_eq!(ch.keys.len(), 3);
        assert_eq!(ch.keys[1].frame, 10.0);
        assert_eq!(ch.keys[1].value, 50.0);
        assert!(
            ch.keys.iter().any(|k| k.in_tan != 0.0 || k.out_tan != 0.0),
            "smoothing should give at least one key a non-flat tangent: {:?}",
            ch.keys
        );
    }

    #[test]
    fn mirror_pose_negates_angles_and_is_its_own_inverse() {
        let mut core = EditorCore::new();
        let before = core.project.frames[0].poses[0].clone();
        assert!(!before.angles.is_empty());
        core.mirror_selected_pose();
        assert!(core.status.contains("Mirrored"), "status: {}", core.status);
        let mirrored = &core.project.frames[0].poses[0];
        assert_eq!(mirrored.root, before.root, "the root must not move");
        for (m, b) in mirrored.angles.iter().zip(&before.angles) {
            assert_eq!(*m, -*b);
        }
        // Mirroring a second time returns the exact original pose.
        core.mirror_selected_pose();
        assert_eq!(core.project.frames[0].poses[0].angles, before.angles);
        assert_eq!(core.project.frames[0].poses[0].root, before.root);
    }

    #[test]
    fn mirror_pose_without_pose_reports_and_changes_nothing() {
        let mut core = EditorCore::new();
        core.project.frames[0].poses.clear();
        core.mirror_selected_pose();
        assert!(core.status.contains("no pose"), "status: {}", core.status);
        assert!(core.project.frames[0].poses.is_empty());
    }

    #[test]
    fn cycle_clip_loop_rotates_through_all_modes() {
        let mut core = EditorCore::new();
        core.project.clips.push(ClipDef {
            name: "T".into(),
            def: FigureDef::simple(),
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        core.cycle_clip_loop(0);
        assert!(matches!(
            core.project.clips[0].loop_mode,
            ClipLoop::PingPong
        ));
        assert!(core.status.contains("ping-pong"), "status: {}", core.status);
        core.cycle_clip_loop(0);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Once));
        assert!(core.status.contains("once"), "status: {}", core.status);
        core.cycle_clip_loop(0);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Loop));
        assert!(core.status.contains("loop"), "status: {}", core.status);
        // Out of range: quiet no-op with a status, mode unchanged.
        core.cycle_clip_loop(9);
        assert!(core.status.contains("No such clip"), "status: {}", core.status);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Loop));
    }
}

#[cfg(test)]
mod wave11_tests {
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::AudioClip;

    /// A one-key RootX channel: `value_at` returns the key value
    /// verbatim at every frame, so baked roots are exact.
    fn rootx_channel(value: f32) -> Channel {
        Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, value)],
        }
    }

    #[test]
    fn bake_graph_channels_writes_channel_poses() {
        let mut core = EditorCore::new();
        let n_frames = core.project.frames.len();
        assert!(n_frames >= 4, "demo project should have several frames");
        let pre_root = core.project.frames[1].poses[0].root;
        core.project.channels.push(rootx_channel(123.5));
        // The expected bake, computed on the pre-bake project.
        let expected: Vec<(f32, Vec<f32>)> = (0..=2)
            .map(|f| {
                let p = crate::graph::apply_channels(&core.project, &core.project.channels, f as f32)
                    .poses[0]
                    .clone();
                (p.root[0], p.angles)
            })
            .collect();
        core.bake_graph_channels(0, 2);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        for (f, (root_x, angles)) in expected.iter().enumerate() {
            let pose = &core.project.frames[f].poses[0];
            assert_eq!(pose.root[0], *root_x, "frame {f} root x should be baked");
            assert_eq!(pose.root[0], 123.5, "frame {f} root x is the channel value");
            assert_eq!(&pose.angles, angles, "frame {f} angles should be baked");
        }
        assert_ne!(pre_root[0], 123.5, "the channel value must differ from the stored root");
        // Frames outside the range keep their stored poses.
        assert_ne!(core.project.frames[3].poses[0].root[0], 123.5);
    }

    #[test]
    fn bake_graph_channels_clamps_the_range_end() {
        let mut core = EditorCore::new();
        let last = core.project.frames.len() - 1;
        core.project.channels.push(rootx_channel(123.5));
        core.bake_graph_channels(0, usize::MAX);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        assert_eq!(core.project.frames[last].poses[0].root[0], 123.5);
    }

    #[test]
    fn bake_graph_channels_noops_without_channels_or_range() {
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let before = core.project.frames[0].poses[0].clone();
        core.bake_graph_channels(0, 2);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert_eq!(core.project.frames[0].poses[0].root, before.root);
        assert_eq!(core.project.frames[0].poses[0].angles, before.angles);

        // With a channel but an invalid range: still no snapshot.
        core.project.channels.push(rootx_channel(123.5));
        core.bake_graph_channels(3, 1);
        assert!(core.status.contains("invalid"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a bad range");
        assert_ne!(core.project.frames[0].poses[0].root[0], 123.5);
    }

    /// Give the demo project a second figure whose frame poses
    /// start as copies of figure 0's.
    fn add_second_figure(core: &mut EditorCore) {
        let fig = core.project.figures[0].clone();
        core.project.figures.push(fig);
        for frame in &mut core.project.frames {
            let pose = frame.poses[0].clone();
            frame.poses.push(pose);
        }
    }

    #[test]
    fn copy_pose_between_copies_root_and_angles_exactly() {
        let mut core = EditorCore::new();
        add_second_figure(&mut core);
        // Make the target pose unmistakably different.
        core.project.frames[0].poses[1].root = [1.0, 2.0];
        for a in &mut core.project.frames[0].poses[1].angles {
            *a = 7.0;
        }
        let source = core.project.frames[0].poses[0].clone();
        core.copy_pose_between(0, 1);
        assert!(core.status.contains("Copied"), "status: {}", core.status);
        let copied = &core.project.frames[0].poses[1];
        assert_eq!(copied.root, source.root);
        assert_eq!(copied.angles, source.angles);
    }

    #[test]
    fn copy_pose_between_rejects_bad_indices() {
        let mut core = EditorCore::new();
        add_second_figure(&mut core);
        let edits = core.edits_since_autosave();
        let before = core.project.frames[0].poses[1].clone();

        core.copy_pose_between(0, 0);
        assert!(core.status.contains("same figure"), "status: {}", core.status);
        core.copy_pose_between(0, 9);
        assert!(core.status.contains("out of range"), "status: {}", core.status);
        core.copy_pose_between(9, 1);
        assert!(core.status.contains("out of range"), "status: {}", core.status);

        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on rejected copies");
        assert_eq!(core.project.frames[0].poses[1].root, before.root);
        assert_eq!(core.project.frames[0].poses[1].angles, before.angles);
    }

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String.
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave11-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`.
    fn core_with_clip(path: String) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame: 0,
            gain: 1.0,
        });
        core
    }

    #[test]
    fn audio_echo_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("echo", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        core.audio_echo_last();
        assert!(core.status.contains("Echo applied"), "status: {}", core.status);
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the echo product");
        let back = crate::audio_edit::read_wav(&new_path).expect("echo product reads back");
        assert!(back.frames() > 0);
        // The original file is untouched and still reads back.
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_lowpass_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("lowpass", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        core.audio_lowpass_last();
        assert!(
            core.status.contains("Low-pass filter"),
            "status: {}",
            core.status
        );
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the low-pass product");
        let back = crate::audio_edit::read_wav(&new_path).expect("low-pass product reads back");
        assert!(back.frames() > 0);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_trim_silence_last_trims_and_reports() {
        let mut samples = vec![0.0; 400];
        samples.extend(vec![0.5; 800]);
        samples.extend(vec![0.0; 400]);
        let src = temp_wav("silence", samples);
        let mut core = core_with_clip(src.clone());
        core.audio_trim_silence_last();
        assert!(core.status.contains("Trimmed"), "status: {}", core.status);
        assert!(
            core.status.contains("800 silent frame(s)"),
            "status: {}",
            core.status
        );
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the trimmed product");
        let back = crate::audio_edit::read_wav(&new_path).expect("trimmed product reads back");
        assert_eq!(back.frames(), 800, "only the audible middle remains");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_trim_silence_last_noops_when_nothing_to_trim() {
        let src = temp_wav("nosilence", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_trim_silence_last();
        assert!(
            core.status.contains("No silence to trim"),
            "status: {}",
            core.status
        );
        assert_eq!(core.project.audio[0].path, src, "the clip keeps its file");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot when nothing changed");
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn audio_effects_report_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.audio_echo_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_lowpass_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_trim_silence_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
    }
}
#[cfg(test)]
mod wave12_tests {
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key, TangentPreset};
    use crate::model::AudioClip;

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String. The `wave12` tag keeps these
    /// files distinct from the wave11 tests' (same pid — one
    /// test binary).
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave12-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`.
    fn core_with_clip(path: String) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame: 0,
            gain: 1.0,
        });
        core
    }

    #[test]
    fn audio_pitch_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("pitch", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_pitch_last();
        assert!(core.status.contains("Pitch shift"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the pitch product");
        assert!(new_path.ends_with(".pitch.wav"), "path: {new_path}");
        let back = crate::audio_edit::read_wav(&new_path).expect("pitch product reads back");
        // +2 semitones resamples by 2^(2/12): 800 frames become
        // ~713 (pitch and duration change together).
        assert!(
            (700..800).contains(&back.frames()),
            "frames: {}",
            back.frames()
        );
        // The original file is untouched and still reads back.
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_compress_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("comp", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_compress_last();
        assert!(core.status.contains("Compressor"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the compressed product");
        assert!(new_path.ends_with(".comp.wav"), "path: {new_path}");
        let back = crate::audio_edit::read_wav(&new_path).expect("compressed product reads back");
        assert_eq!(back.frames(), 800, "compression keeps the frame count");
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_pitch_and_compress_report_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let edits = core.edits_since_autosave();
        core.audio_pitch_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_compress_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot without clips");
    }

    /// A stroke literal in the shape `begin_draw` records.
    fn stroke(frame: usize, points: Vec<[f32; 2]>) -> crate::model::Stroke {
        crate::model::Stroke {
            frame,
            points,
            color: 0x112233,
            width: 3.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
        }
    }

    #[test]
    fn mirror_strokes_x_mirrors_current_frame_only() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let cur = core.current;
        let w = core.project.stage[0];
        core.project.strokes.push(stroke(cur, vec![[10.0, 20.0], [30.0, 40.0]]));
        core.project.strokes.push(stroke(cur + 1, vec![[10.0, 20.0]]));
        let edits = core.edits_since_autosave();
        core.mirror_strokes_x();
        assert!(
            core.status.contains("Mirrored 1 stroke(s)"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // x' = stage width - x; y unchanged; order preserved.
        assert_eq!(
            core.project.strokes[0].points,
            vec![[w - 10.0, 20.0], [w - 30.0, 40.0]]
        );
        // The other frame's stroke is untouched.
        assert_eq!(core.project.strokes[1].points, vec![[10.0, 20.0]]);
        // Mirroring again restores the original points exactly.
        core.mirror_strokes_x();
        assert_eq!(core.project.strokes[0].points, vec![[10.0, 20.0], [30.0, 40.0]]);
    }

    #[test]
    fn mirror_strokes_x_noops_without_strokes_on_the_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        // A stroke on ANOTHER frame does not count.
        core.project.strokes.push(stroke(core.current + 1, vec![[1.0, 2.0]]));
        let edits = core.edits_since_autosave();
        core.mirror_strokes_x();
        assert!(
            core.status.contains("No strokes to mirror"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
    }

    #[test]
    fn apply_graph_ease_preset_shapes_selected_figure_only() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let keys = || {
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 0.0),
            ]
        };
        core.project.channels.push(Channel {
            figure: fi,
            target: ChannelTarget::Angle(0),
            keys: keys(),
        });
        core.project.channels.push(Channel {
            figure: fi + 1,
            target: ChannelTarget::Angle(0),
            keys: keys(),
        });
        let edits = core.edits_since_autosave();
        core.apply_graph_ease_preset(TangentPreset::Smooth);
        assert!(core.status.contains("Smooth"), "status: {}", core.status);
        assert!(
            core.status.contains("1 graph channel(s)"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Smooth is Catmull-Rom: the first key's tangents are the
        // one-sided secant (50 - 0) / (10 - 0) = 5.
        assert_eq!(core.project.channels[0].keys[0].out_tan, 5.0);
        assert_eq!(core.project.channels[0].keys[0].in_tan, 5.0);
        // The other figure's channel keeps its flat tangents.
        assert_eq!(core.project.channels[1].keys[0].out_tan, 0.0);
        assert_eq!(core.project.channels[1].keys[0].in_tan, 0.0);
    }

    #[test]
    fn apply_graph_ease_preset_noops_without_channels_for_the_figure() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        let edits = core.edits_since_autosave();
        core.apply_graph_ease_preset(TangentPreset::EaseInOut);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        // A channel belonging to another figure does not count.
        core.project.channels.push(Channel {
            figure: core.selected_figure + 1,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0), Key::flat(10.0, 10.0)],
        });
        core.apply_graph_ease_preset(TangentPreset::EaseInOut);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
    }

    /// Assert the shared stamp state on the last stroke of a
    /// core whose strokes were cleared: current frame, selected
    /// figure's colour, active brush width (per point too), no
    /// fill.
    fn check_stamp(core: &EditorCore) -> &crate::model::Stroke {
        assert_eq!(core.project.strokes.len(), 1, "exactly one stroke stamped");
        let s = core.project.strokes.last().unwrap();
        assert_eq!(s.frame, core.current);
        assert_eq!(s.color, core.project.figures[core.selected_figure].color);
        assert_eq!(s.width, core.active_brush().width);
        assert_eq!(s.fill, None);
        assert_eq!(s.widths.len(), s.points.len(), "a width per point");
        assert!(s.widths.iter().all(|&x| x == s.width));
        s
    }

    #[test]
    fn stamp_polygon_appends_a_pentagon_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (w, h) = (core.project.stage[0], core.project.stage[1]);
        let edits = core.edits_since_autosave();
        core.stamp_polygon();
        assert!(
            core.status.contains("Stamped a pentagon"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // Closed pentagon: 5 vertices + the closing repeat, the
        // first vertex pointing up from the centre (radius 60).
        assert_eq!(s.points.len(), 6);
        let first = s.points[0];
        assert!((first[0] - w / 2.0).abs() < 0.001, "first x: {}", first[0]);
        assert!(
            (first[1] - (h / 2.0 - 60.0)).abs() < 0.001,
            "first y: {}",
            first[1]
        );
        assert_eq!(s.points[5], s.points[0], "closed: last repeats first");
    }

    #[test]
    fn stamp_star_appends_a_five_pointed_star_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (cx, cy) = (core.project.stage[0] / 2.0, core.project.stage[1] / 2.0);
        let edits = core.edits_since_autosave();
        core.stamp_star();
        assert!(core.status.contains("Stamped a star"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // 2 * 5 alternating outer/inner vertices + closing repeat.
        assert_eq!(s.points.len(), 11);
        assert_eq!(s.points[10], s.points[0], "closed: last repeats first");
        // Every vertex sits on the outer (60) or inner (26) ring.
        for p in &s.points {
            let d = ((p[0] - cx) * (p[0] - cx) + (p[1] - cy) * (p[1] - cy)).sqrt();
            assert!(
                (d - 60.0).abs() < 0.01 || (d - 26.0).abs() < 0.01,
                "vertex distance from centre: {d}"
            );
        }
    }

    #[test]
    fn stamp_arrow_appends_a_right_pointing_arrow_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (cx, cy) = (core.project.stage[0] / 2.0, core.project.stage[1] / 2.0);
        let edits = core.edits_since_autosave();
        core.stamp_arrow();
        assert!(
            core.status.contains("Stamped an arrow"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // [tail, tip, wing, tip, wing] — the shaft endpoints are
        // the caller's points verbatim.
        assert_eq!(s.points.len(), 5);
        assert_eq!(s.points[0], [cx - 70.0, cy]);
        assert_eq!(s.points[1], [cx + 70.0, cy]);
        assert_eq!(s.points[3], s.points[1], "the polyline revisits the tip");
    }
}
#[cfg(test)]
mod wave13_tests {
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::model::AudioClip;

    /// Select a brush preset by name (the draw tests in
    /// `mod tests` pick brushes the same way).
    fn set_brush(core: &mut EditorCore, name: &str) {
        core.brush_index = crate::paint::brush_library()
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("no brush named {name}"));
    }

    #[test]
    fn spray_brush_records_a_deterministic_spray_mark() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        set_brush(&mut core, "Spray");
        let brush = core.active_brush();
        assert!(brush.scatter > 0.0, "the Spray preset must scatter");
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        let s = &core.project.strokes[0];
        // The recorded points stay the drawn path — the Spray
        // preset has no smoothing and a round nib, so neither
        // end-of-stroke step rewrites them; the mark is what
        // tells renderers to scatter dots along it.
        assert_eq!(s.points, vec![[0.0, 0.0], [30.0, 0.0]]);
        let mark = s.spray.expect("a scatter brush records a spray mark");
        assert_eq!(mark.seed, 0x5EED_0000u64, "frame 0, stroke index 0");
        assert_eq!(mark.radius, brush.scatter);
        assert_eq!(mark.opacity, brush.opacity);

        // A second spray stroke on a later frame: the seed
        // moves with both the frame (x131) and the stroke's
        // own index (+1).
        core.current = 3;
        core.begin_draw([10.0, 10.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 2);
        let mark = core.project.strokes[1]
            .spray
            .expect("the second spray stroke is marked too");
        assert_eq!(mark.seed, 0x5EED_0000u64 + 3 * 131 + 1);
    }

    #[test]
    fn plain_brush_records_no_spray_mark() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        assert_eq!(core.active_brush().name, "Pen");
        assert_eq!(core.active_brush().scatter, 0.0);
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        assert!(core.project.strokes[0].spray.is_none());
    }

    fn fx_entry(kind: crate::vfx::ClipKind, seed: u32) -> crate::model::FxClipEntry {
        crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 16.0,
                kind,
                color: 0xFFCC00,
                seed,
                angle: 0.0,
                length: 130.0,
                color2: None,
            },
            figure: 0,
            node: 0,
        }
    }

    #[test]
    fn fx_preset_save_then_apply_round_trips_the_clip() {
        let mut core = EditorCore::new();
        core.project.fx_clips.clear();
        core.project
            .fx_clips
            .push(fx_entry(crate::vfx::ClipKind::Sparkle, 42));
        let edits = core.edits_since_autosave();

        // Saving writes the preset file but is not an undoable
        // project change: no snapshot.
        core.fx_save_preset(0);
        assert!(
            core.status.contains("FX preset 'sparkle-1' saved"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "save takes no snapshot");
        assert!(
            core.fx_preset_names().iter().any(|n| n == "sparkle-1"),
            "the saved preset is listed"
        );

        // Applying adds a fresh entry at the current frame on
        // the selected figure (node 0), as one undo step.
        core.current = 4;
        core.fx_apply_preset("sparkle-1");
        assert!(
            core.status.contains("added at frame 5"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.fx_clips.len(), 2);
        let added = &core.project.fx_clips[1];
        assert!(matches!(added.clip.kind, crate::vfx::ClipKind::Sparkle));
        assert_eq!(added.clip.seed, 42, "the preset's clip data round-trips");
        assert_eq!(added.clip.start, 4.0);
        assert_eq!(added.figure, core.selected_figure);
        assert_eq!(added.node, 0);

        let _ = std::fs::remove_file(
            std::path::PathBuf::from("fx-presets").join("sparkle-1.json"),
        );
    }

    #[test]
    fn fx_preset_failures_change_nothing() {
        let mut core = EditorCore::new();
        core.project.fx_clips.clear();
        let edits = core.edits_since_autosave();
        core.fx_save_preset(0);
        assert!(
            core.status.contains("no FX clip 1"),
            "status: {}",
            core.status
        );
        core.fx_apply_preset("wave13-no-such-preset");
        assert!(
            core.status.contains("FX preset apply failed"),
            "status: {}",
            core.status
        );
        assert!(core.project.fx_clips.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on failures");
    }

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String. The `wave13` tag keeps these
    /// files distinct from the wave11/12 tests' (same pid — one
    /// test binary).
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave13-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`,
    /// starting at `start_frame`, with the project at `fps`.
    fn core_with_clip(path: String, start_frame: usize, fps: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame,
            gain: 1.0,
        });
        core
    }

    #[test]
    fn audio_split_at_silences_last_splits_two_bursts() {
        // 0.5s burst, 0.5s silence, 0.5s burst at 8 kHz: the
        // gap (4000 frames) clears the 0.25s minimum (2000
        // frames), so the sounding ranges are exactly the two
        // bursts.
        let mut samples = vec![0.5; 4000];
        samples.extend(vec![0.0; 4000]);
        samples.extend(vec![0.5; 4000]);
        let src = temp_wav("bursts", samples);
        let mut core = core_with_clip(src.clone(), 10, 30.0);
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("split at silences into 2 clips"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.audio.len(), 2);
        let (first, second) = (&core.project.audio[0], &core.project.audio[1]);
        assert!(first.path.ends_with(".seg1.wav"), "path: {}", first.path);
        assert!(second.path.ends_with(".seg2.wav"), "path: {}", second.path);
        // Segment 1 starts where the clip started; segment 2 is
        // offset by its range start (8000 frames = 1.0s = 30
        // frames at 30 fps).
        assert_eq!(first.start_frame, 10);
        assert_eq!(second.start_frame, 40);
        assert!(first.start_frame < second.start_frame, "starts are ordered");
        for clip in [first, second] {
            let back =
                crate::audio_edit::read_wav(&clip.path).expect("segment file reads back");
            assert_eq!(back.frames(), 4000, "each burst is one segment");
        }
        // The original file is untouched.
        assert_eq!(crate::audio_edit::read_wav(&src).unwrap().frames(), 12000);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&first.path);
        let _ = std::fs::remove_file(&second.path);
    }

    #[test]
    fn audio_split_at_silences_last_noops_without_a_long_gap() {
        // Continuous sound is a single range: nothing to split,
        // no files written, no snapshot.
        let src = temp_wav("steady", vec![0.5; 8000]);
        let mut core = core_with_clip(src.clone(), 7, 30.0);
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("no silence of 0.25s or more"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.project.audio[0].path, src);
        assert_eq!(core.project.audio[0].start_frame, 7);
        let source_path = std::path::PathBuf::from(&src);
        let stem = source_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let seg1 = source_path.with_file_name(format!("{stem}.seg1.wav"));
        assert!(!seg1.exists(), "no segment file is written on a no-op");
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn audio_split_at_silences_last_reports_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("no audio clips"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot without clips");
    }
}

#[cfg(test)]
mod wave14_camera_tests {
    use crate::editor::EditorCore;
    use crate::model::{
        CameraKey, FigureDef, FigureInstance, Frame, NodeDef, NodeStyle, Pose, Shape,
    };

    /// Two `FigureDef::simple()` figures with all-zero pose
    /// angles. With every angle 0, FK points every segment
    /// straight down (model::dir(0) = [0, 1]), so each figure's
    /// joints lie on the vertical line through its root, spanning
    /// root.y ..= root.y + 118 (the simple rig's longest
    /// root-to-node chain: hips -> chest 52 -> arm 66 = 118).
    /// Alpha's root is [100, 100], Beta's is [500, 300]; the
    /// demo stage (960 x 720) is kept.
    fn two_figure_core(frames: usize) -> EditorCore {
        let mut core = EditorCore::new();
        let def = FigureDef::simple();
        let fig = |name: &str| FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: def.clone(),
            hidden: false,
            clip: None,
            locked: false,
            opacity: 1.0,
        };
        core.project.figures = vec![fig("Alpha"), fig("Beta")];
        let frame = || Frame {
            poses: vec![
                Pose {
                    root: [100.0, 100.0],
                    angles: vec![0.0; 7],
                },
                Pose {
                    root: [500.0, 300.0],
                    angles: vec![0.0; 7],
                },
            ],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        };
        core.project.frames = (0..frames).map(|_| frame()).collect();
        core.project.camera = Vec::new();
        core.project.camera_follow = None;
        core.current = 0;
        core.selected_figure = 0;
        core
    }

    #[test]
    fn frame_all_fits_two_figures_with_ten_percent_margin() {
        let mut core = two_figure_core(1);
        let edits = core.edits_since_autosave();
        core.frame_all_figures();
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.camera.len(), 1);
        let k = &core.project.camera[0];
        assert_eq!(k.frame, 0);
        // Joint bounds: x 100..=500, y 100..=418 -> centre (300, 259).
        assert!((k.x - 300.0).abs() < 1e-3, "x: {}", k.x);
        assert!((k.y - 259.0).abs() < 1e-3, "y: {}", k.y);
        // Derivation: the view shows stage/zoom stage units around
        // the camera centre (the view closure in export.rs
        // render_frame_at; the canvas map in app.rs paint_stage
        // agrees), so fitting bounds bw x bh needs
        // zoom <= stage_w / bw and zoom <= stage_h / bh. Here
        // 960/400 = 2.4 and 720/318 = 2.26415 — the height binds —
        // and the 10% margin divides by 1.1.
        let expected = (720.0f32 / 318.0) / 1.1;
        assert!((k.zoom - expected).abs() < 1e-4, "zoom {} vs {expected}", k.zoom);
        assert_eq!(k.angle, 0.0);
        // The evaluated camera at frame 0 is the key itself.
        assert_eq!(core.project.camera_at(0.0), (k.x, k.y, k.zoom));
    }

    #[test]
    fn frame_all_ignores_hidden_figures() {
        let mut core = two_figure_core(1);
        core.project.figures[1].hidden = true;
        core.frame_all_figures();
        let k = &core.project.camera[0];
        // Only Alpha counts: bounds x = {100} (zero width imposes
        // no constraint), y 100..=218 -> centre (100, 159), zoom
        // from the height alone.
        assert!((k.x - 100.0).abs() < 1e-3, "x: {}", k.x);
        assert!((k.y - 159.0).abs() < 1e-3, "y: {}", k.y);
        let expected = (720.0f32 / 118.0) / 1.1;
        assert!((k.zoom - expected).abs() < 1e-4, "zoom {} vs {expected}", k.zoom);
    }

    #[test]
    fn frame_all_preserves_the_current_angle_when_inserting_a_key() {
        let mut core = two_figure_core(3);
        core.project.camera = vec![CameraKey {
            frame: 0,
            x: 480.0,
            y: 360.0,
            zoom: 1.0,
            angle: 30.0,
        }];
        core.current = 2;
        core.frame_all_figures();
        // The fresh key at frame 2 must carry the angle in effect
        // there (30, held from frame 0's key), not the 0.0 that
        // set_camera_key gives a new key.
        let k = core
            .project
            .camera
            .iter()
            .find(|k| k.frame == 2)
            .expect("key at frame 2");
        assert_eq!(k.angle, 30.0);
        assert_eq!(core.project.camera_angle_at(2.0), 30.0);
    }

    #[test]
    fn frame_all_noops_without_figures() {
        let mut core = two_figure_core(1);
        core.project.figures = Vec::new();
        let edits = core.edits_since_autosave();
        core.frame_all_figures();
        assert_eq!(core.status, "No figures to frame.");
        assert!(core.project.camera.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot");
    }

    #[test]
    fn frame_all_single_point_bounds_keeps_current_zoom() {
        let mut core = two_figure_core(1);
        core.project.figures = vec![FigureInstance {
            name: "Dot".into(),
            color: 0xFFFFFF,
            def: FigureDef {
                nodes: vec![NodeDef {
                    name: "origin".into(),
                    parent: None,
                    length: 0.0,
                    rest_angle: 0.0,
                    head: false,
                    bend: 1.0,
                    thickness: 6.0,
                    color: None,
                    ik_end: false,
                    shape: Shape::Line,
                    style: NodeStyle::default(),
                    media: None,
                }],
            },
            hidden: false,
            clip: None,
            locked: false,
            opacity: 1.0,
        }];
        core.project.frames[0].poses = vec![Pose {
            root: [200.0, 200.0],
            angles: vec![0.0],
        }];
        core.frame_all_figures();
        let k = &core.project.camera[0];
        assert_eq!((k.x, k.y), (200.0, 200.0));
        // No camera keys existed, so the current zoom is the
        // no-keys default 1.0, and a point bounds must keep it.
        assert_eq!(k.zoom, 1.0);
    }

    #[test]
    fn centre_on_selection_centres_on_the_selected_root_and_keeps_zoom() {
        let mut core = two_figure_core(1);
        core.project.camera = vec![CameraKey {
            frame: 0,
            x: 480.0,
            y: 360.0,
            zoom: 2.0,
            angle: 0.0,
        }];
        core.selected_figure = 1;
        let edits = core.edits_since_autosave();
        core.centre_on_selection();
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(
            core.project.camera.len(),
            1,
            "the key at frame 0 is replaced, not duplicated"
        );
        let k = &core.project.camera[0];
        // Beta's root, with the pre-existing zoom untouched.
        assert_eq!((k.x, k.y, k.zoom), (500.0, 300.0, 2.0));
        assert!(core.status.contains("Beta"), "status: {}", core.status);
    }

    #[test]
    fn centre_on_selection_noops_without_figures() {
        let mut core = two_figure_core(1);
        core.project.figures = Vec::new();
        let edits = core.edits_since_autosave();
        core.centre_on_selection();
        assert_eq!(core.status, "No figures to centre on.");
        assert!(core.project.camera.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot");
    }
}

#[cfg(test)]
mod wave15_graph_interp_tests {
    use super::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Interp, Key};

    /// A core whose selected figure has one angle channel for
    /// joint 0 with Hermite keys at frames 0, 10 and 20, and
    /// joint 0 selected.
    fn core_with_angle_keys() -> EditorCore {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.selected_node = Some(0);
        core.project.channels.clear();
        core.project.channels.push(Channel {
            figure: fi,
            target: ChannelTarget::Angle(0),
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 0.0),
            ],
        });
        core
    }

    #[test]
    fn set_graph_key_interp_changes_only_the_key_at_the_current_frame() {
        let mut core = core_with_angle_keys();
        core.current = 10;
        let edits = core.edits_since_autosave();
        core.set_graph_key_interp(Interp::Linear);
        assert!(core.status.contains("Linear"), "status: {}", core.status);
        assert!(core.status.contains("frame 11"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[0].interp, Interp::Hermite, "earlier key untouched");
        assert_eq!(keys[1].interp, Interp::Linear, "the key at the current frame changed");
        assert_eq!(keys[2].interp, Interp::Hermite, "later key untouched");
    }

    #[test]
    fn set_graph_key_interp_picks_the_nearest_key_to_the_current_frame() {
        let mut core = core_with_angle_keys();
        // 17 is 3 frames from the key at 20 and 7 from the key at 10.
        core.current = 17;
        core.set_graph_key_interp(Interp::Step);
        assert!(core.status.contains("frame 21"), "status: {}", core.status);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[2].interp, Interp::Step, "nearest key changed");
        assert_eq!(keys[1].interp, Interp::Hermite, "farther key untouched");
        assert_eq!(keys[0].interp, Interp::Hermite, "farthest key untouched");
        // A tie goes to the earlier key: 15 is equidistant from
        // the keys at 10 and 20.
        core.current = 15;
        core.set_graph_key_interp(Interp::Linear);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[1].interp, Interp::Linear, "earlier key wins the tie");
        assert_eq!(keys[2].interp, Interp::Step, "later key keeps its mode");
    }

    #[test]
    fn set_graph_key_interp_noops_without_a_joint_or_keys() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        core.selected_node = None;
        let edits = core.edits_since_autosave();
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("Select a joint"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        // Joint selected but no channel for it.
        core.selected_node = Some(0);
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("No graph keys"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
        // A channel for another target does not count.
        core.project.channels.push(Channel {
            figure: core.selected_figure,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0)],
        });
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("No graph keys"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
    }
}
