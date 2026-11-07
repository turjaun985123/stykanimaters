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
    clipboard: Option<crate::model::Frame>,
    pub playhead: f32,
    pub show_paths: bool,
    pub light_theme: bool,
    pub library: Vec<(String, FigureDef)>,
    pub library_index: usize,
    /// Draw mode: canvas drags paint brush strokes on this frame.
    pub draw_mode: bool,
    drawing: bool,
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
            clipboard: None,
            playhead: 0.0,
            show_paths: false,
            light_theme: false,
            library: Self::load_library(),
            library_index: 0,
            draw_mode: false,
            drawing: false,
            onion_frames: 1,
            project_files: Self::list_project_files(),
            project_index: 0,
            lang: crate::i18n::Lang::En,
            fig3: None,
            solo: None,
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
            .insert(self.current + 1, crate::model::Frame { poses, duration, easing: None });
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
            crate::model::Frame { poses, duration: 1.0, easing: None },
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
    /// per-frame durations. Loops at the end.
    pub fn advance_play(&mut self, dt_secs: f32) {
        let n = self.project.frames.len();
        if n == 0 {
            return;
        }
        let duration = self.project.frames[self.current].duration.max(0.05);
        self.playhead += dt_secs * self.project.fps.max(1.0) / duration;
        while self.playhead >= n as f32 {
            self.playhead -= n as f32;
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
        self.project
            .particles
            .retain(|e| e.figure.map(|f| f != fi).unwrap_or(true));
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
                gain: 1.0,
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
            },
            figure: self.selected_figure,
            node,
        });
        let _ = ClipKind::Burst;
        self.status = format!("FX clip ({kind:?}) added at frame {}.", self.current + 1);
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

    // ---- Drawing ----

    pub fn begin_draw(&mut self, p: V2) {
        let color = self.project.figures[self.selected_figure].color;
        self.project.strokes.push(crate::model::Stroke {
            frame: self.current,
            points: vec![p],
            color,
            width: 4.0,
            widths: vec![4.0],
            fill: None,
        });
        self.drawing = true;
    }

    pub fn draw_to(&mut self, p: V2) {
        if !self.drawing {
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
        if self.drawing {
            self.drawing = false;
            if let Some(stroke) = self.project.strokes.last_mut() {
                let n = stroke.points.len();
                self.status = format!("Stroke drawn ({n} points).");
            }
        }
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

    /// Bake the current 3D pose into a new 2D figure in the project.
    /// Projected segment lengths and directions become the figure
    /// definition (origin node + one node per 3D joint, parent
    /// structure preserved, thickness scaled by projected scale —
    /// the same conversion the file importers use). The pose is
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
            let mut abs = vec![0.0f32; baked.len()];
            let mut nodes = Vec::with_capacity(baked.len());
            for (i, bn) in baked.iter().enumerate() {
                let n3 = &st.figure.nodes[i];
                abs[i] = bn.abs_angle;
                let parent_abs = n3.parent.map(|p| abs[p]).unwrap_or(0.0);
                nodes.push(NodeDef {
                    name: n3.name.clone(),
                    parent: n3.parent,
                    length: bn.length,
                    rest_angle: crate::model::norm_deg(bn.abs_angle - parent_abs),
                    head: n3.head,
                    bend: 1.0,
                    thickness: bn.thickness,
                    color: None,
                    ik_end: false,
                    shape: crate::model::Shape::Line,
                    style: crate::model::NodeStyle::default(),
                    media: None,
                });
            }
            let root = baked.first().map(|b| b.xy).unwrap_or([0.0, 0.0]);
            (
                FigureDef { nodes },
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
        });
        core.project.audio.push(crate::model::AudioClip {
            path: other.clone(),
            start_frame: 30,
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
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 3,
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
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 15,
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
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 10,
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
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 3,
        });

        core.audio_crossfade(0);

        assert_eq!(core.project.audio.len(), 2);
        assert_audio_clip(&core, 0, &first, 0);
        assert_audio_clip(&core, 1, &second, 3);
        assert!(!dir.join("a.xfade.wav").exists());
        assert!(core.status.contains("Audio crossfade failed"), "status: {}", core.status);
    }
}
