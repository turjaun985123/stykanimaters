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

    fn snapshot(&mut self) {
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
            .insert(self.current + 1, crate::model::Frame { poses, duration });
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
            crate::model::Frame { poses, duration: 1.0 },
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
        let poses = self.project.poses_at(frame_f);
        let fig = &self.project.figures[fi];
        evaluate(&fig.def, &poses[fi])
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
        self.status = "Particle effects cleared.".into();
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
        });
        self.drawing = true;
    }

    pub fn draw_to(&mut self, p: V2) {
        if !self.drawing {
            return;
        }
        if let Some(stroke) = self.project.strokes.last_mut() {
            let last = *stroke.points.last().unwrap();
            if dist(last, p) > 2.0 {
                stroke.points.push(p);
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
}
