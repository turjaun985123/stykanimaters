//! GPUI view: toolbar, canvas (figure rendering + mouse posing) and
//! the frame timeline. All document logic lives in `editor.rs`.

use crate::editor::EditorCore;
use crate::model::V2;
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct EditorView {
    core: EditorCore,
    canvas_bounds: Rc<RefCell<Bounds<Pixels>>>,
    focus_handle: FocusHandle,
    player: crate::audio::AudioPlayer,
    /// When true, the home panel (saved projects + recents)
    /// replaces the canvas area.
    show_home: bool,
}

impl EditorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        EditorView {
            core: EditorCore::new(),
            canvas_bounds: Rc::new(RefCell::new(Bounds::default())),
            focus_handle: cx.focus_handle(),
            player: crate::audio::AudioPlayer::new(),
            show_home: false,
        }
    }

    /// Open a project chosen on the home panel, routed through
    /// the editor's existing cycle+open flow: select the matching
    /// entry in `core.project_files` (matched by file name — the
    /// library scan reports relative paths, the core absolute
    /// ones), adding it first if the core has not listed it yet.
    fn open_home_project(&mut self, path: &std::path::Path, name: &str) {
        let fname = path.file_name();
        if let Some(idx) = self
            .core
            .project_files
            .iter()
            .position(|p| p.file_name() == fname)
        {
            self.core.project_index = idx;
        } else {
            self.core.project_files.push(path.to_path_buf());
            self.core.project_index = self.core.project_files.len() - 1;
        }
        self.core.open_project_file();
        crate::shell::touch_recent(name);
        self.show_home = false;
    }

    /// Resize the stage by (dw, dh), each axis clamped to
    /// 200..=4000, reporting the new size in the status line.
    fn adjust_stage(&mut self, dw: f32, dh: f32) {
        let core = &mut self.core;
        core.project.stage[0] = (core.project.stage[0] + dw).clamp(200.0, 4000.0);
        core.project.stage[1] = (core.project.stage[1] + dh).clamp(200.0, 4000.0);
        core.status = format!(
            "Stage size: {} x {}.",
            core.project.stage[0] as i32, core.project.stage[1] as i32
        );
    }

    /// (scale, cam centre) of the canvas view: fit-to-canvas times
    /// the frame's camera zoom, centred on the camera centre.
    fn view_transform(&self) -> (f32, V2) {
        let b = *self.canvas_bounds.borrow();
        let stage = self.core.project.stage;
        let (cx, cy, zoom) = self.core.camera_here();
        let fit = (f32::from(b.size.width) / stage[0]).min(f32::from(b.size.height) / stage[1]);
        (fit * zoom, [cx, cy])
    }

    fn to_stage(&self, window_pos: Point<Pixels>) -> V2 {
        let b = *self.canvas_bounds.borrow();
        let (scale, cam) = self.view_transform();
        let dx = f32::from(window_pos.x - b.origin.x) - f32::from(b.size.width) / 2.0;
        let dy = f32::from(window_pos.y - b.origin.y) - f32::from(b.size.height) / 2.0;
        [cam[0] + dx / scale, cam[1] + dy / scale]
    }

    fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.core.playing = !self.core.playing;
        if self.core.playing {
            // Start the audio clip that is audible at the playhead
            // (multi-clip aware; simultaneous clips are covered by
            // the Mixdown export - playback takes the first audible).
            {
                let fps = self.core.project.fps.max(1.0);
                let playhead = self.core.current as f32 / fps;
                let durations = |path: &str| {
                    crate::audio_edit::read_wav(path).ok().map(|w| w.duration_secs())
                };
                let audible = crate::audio_edit::audible_at(
                    &self.core.project.audio,
                    playhead,
                    fps,
                    &durations,
                );
                if let Some(&(ci, offset)) = audible.first() {
                    let clip = &self.core.project.audio[ci];
                    let _ = self.player.play(&clip.path, offset);
                }
            }
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(33))
                        .await;
                    let keep = this
                        .update(cx, |view, cx| {
                            if view.core.playing {
                                view.core.advance_play(0.033);
                                if !view.core.playing {
                                    view.player.stop();
                                }
                                cx.notify();
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
            .detach();
        } else {
            self.player.stop();
        }
        cx.notify();
    }
}

/// Everything the canvas paint closure needs, cloned out of the core.
struct DrawData {
    stage: [f32; 2],
    background: u32,
    /// (segments (a, b, colour, thickness), joints, heads, figure colour)
    figures: Vec<(Vec<(V2, V2, u32, f32)>, Vec<V2>, Vec<V2>, u32)>,
    /// (centre, shape, length, colour, thickness) shape segments
    shapes: Vec<(V2, crate::model::Shape, f32, u32, f32)>,
    /// Onion skin entries (segments, colour, alpha) covering BOTH
    /// directions around the current frame: previous frames keep
    /// each figure's own colour, while next frames all use one
    /// shared green tint (0x7BE495) so the animator can tell at a
    /// glance where a pose came from versus where it is heading.
    onion: Vec<(Vec<(V2, V2)>, u32, u8)>,
    selected: Option<(usize, usize)>,
    cam: (f32, f32, f32),
    /// Motion paths: one polyline per visible figure (root), plus the
    /// selected joint's path appended last when enabled.
    paths: Vec<Vec<V2>>,
    particles: Vec<(V2, f32, u32, f32)>,
    strokes: Vec<(Vec<V2>, u32, f32)>,
}

impl EditorView {
    fn draw_data(&self) -> DrawData {
        let core = &self.core;
        let display = core.display_frame();
        let mut figures = Vec::new();
        let mut shapes = Vec::new();
        for fi in 0..core.project.figures.len() {
            if core.project.figures[fi].hidden {
                figures.push((Vec::new(), Vec::new(), Vec::new(), core.project.figures[fi].color));
                continue;
            }
            let pos = core.positions_f(display, fi);
            let def = &core.project.figures[fi].def;
            let mut segs = Vec::new();
            let mut heads = Vec::new();
            let fig_color = core.project.figures[fi].color;
            for (i, node) in def.nodes.iter().enumerate() {
                if let Some(p) = node.parent {
                    segs.push((pos[p], pos[i], node.color.unwrap_or(fig_color), node.thickness));
                }
                if node.head {
                    heads.push(pos[i]);
                }
            }
            for (i, node) in def.nodes.iter().enumerate() {
                if !matches!(node.shape, crate::model::Shape::Line) {
                    shapes.push((pos[i], node.shape, node.length, node.color.unwrap_or(fig_color), node.thickness));
                }
            }
            figures.push((segs, pos, heads, fig_color));
        }
        let mut onion = Vec::new();
        if core.onion && core.current > 0 {
            for fi in 0..core.project.figures.len() {
                if core.project.figures[fi].hidden {
                    continue;
                }
                let pos = core.positions(core.current - 1, fi);
                let def = &core.project.figures[fi].def;
                let mut segs = Vec::new();
                for (i, node) in def.nodes.iter().enumerate() {
                    if let Some(p) = node.parent {
                        segs.push((pos[p], pos[i]));
                    }
                }
                onion.push((segs, core.project.figures[fi].color, 0x4D));
            }
            // Older onion frames, fainter.
            for back in 2..=(core.onion_frames as usize) {
                if core.current < back {
                    break;
                }
                for fi in 0..core.project.figures.len() {
                    if core.project.figures[fi].hidden {
                        continue;
                    }
                    let pos = core.positions(core.current - back, fi);
                    let def = &core.project.figures[fi].def;
                    let mut segs = Vec::new();
                    for (i, node) in def.nodes.iter().enumerate() {
                        if let Some(p) = node.parent {
                            segs.push((pos[p], pos[i]));
                        }
                    }
                    onion.push((segs, core.project.figures[fi].color, 0x26));
                }
            }
        }
        // Next frames (where the pose is heading), colour-coded with
        // one green tint for every figure, mirroring the previous
        // frames' alpha falloff.
        if core.onion {
            for ahead in 1..=(core.onion_frames as usize) {
                let frame = core.current + ahead;
                if frame >= core.frame_count() {
                    break;
                }
                let alpha = if ahead == 1 { 0x4D } else { 0x26 };
                for fi in 0..core.project.figures.len() {
                    if core.project.figures[fi].hidden {
                        continue;
                    }
                    let pos = core.positions(frame, fi);
                    let def = &core.project.figures[fi].def;
                    let mut segs = Vec::new();
                    for (i, node) in def.nodes.iter().enumerate() {
                        if let Some(p) = node.parent {
                            segs.push((pos[p], pos[i]));
                        }
                    }
                    onion.push((segs, 0x7BE495, alpha));
                }
            }
        }
        let mut paths = Vec::new();
        if core.show_paths {
            for fi in 0..core.project.figures.len() {
                if core.project.figures[fi].hidden {
                    continue;
                }
                paths.push(
                    (0..core.frame_count())
                        .map(|f| core.positions(f, fi)[0])
                        .collect::<Vec<V2>>(),
                );
            }
            if let Some(ni) = core.selected_node {
                let fi = core.selected_figure;
                if ni < core.project.figures[fi].def.nodes.len() {
                    paths.push(
                        (0..core.frame_count())
                            .map(|f| core.positions(f, fi)[ni])
                            .collect::<Vec<V2>>(),
                    );
                }
            }
        }
        let mut particles: Vec<(V2, f32, u32, f32)> = crate::export::particles_at(&core.project, display)
            .into_iter()
            .map(|(p, r, c, a)| (p, r, ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32, a))
            .collect();
        for entry in &core.project.fx_clips {
            if entry.figure < core.project.figures.len() && !core.project.figures[entry.figure].hidden {
                let joints = core.positions_f(display, entry.figure);
                if let Some(&anchor) = joints.get(entry.node) {
                    for (p, r, c, a) in crate::vfx::clip_particles(&entry.clip, display, anchor) {
                        particles.push((p, r, ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32, a));
                    }
                }
            }
        }
        let strokes = core
            .project
            .strokes
            .iter()
            .filter(|s| s.frame == core.current)
            .map(|s| (s.points.clone(), s.color, s.width))
            .collect();
        DrawData {
            stage: core.project.stage,
            background: core.project.background,
            figures,
            shapes,
            onion,
            selected: self.core.selected_node.map(|n| (self.core.selected_figure, n)),
            cam: core.project.camera_at(display),
            paths,
            particles,
            strokes,
        }
    }
}

fn btn(
    id: impl Into<ElementId>,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .bg(if active { rgb(0x3D5A99) } else { rgb(0x2C313A) })
        .hover(|s| s.bg(rgb(0x3A4150)))
        .text_size(px(13.0))
        .text_color(rgb(0xE8E8EC))
        .on_click(on_click)
        .child(label)
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.focus(&self.focus_handle);
        let core = &self.core;
        let frame_label = format!("Frame {} / {}", core.current + 1, core.frame_count());
        let fps_label = format!("{} fps", core.project.fps as i32);
        let status = core.status.clone();
        let playing = core.playing;
        let onion_on = core.onion;
        let cam_mode = core.camera_mode;
        let draw_on = core.draw_mode;
        let (ccx, ccy, czoom) = core.camera_here();
        let cam_label = format!("cam ({ccx:.0}, {ccy:.0}) x{czoom:.2} · {} key(s)", core.project.camera.len());
        let (root_bg, bar_bg, row_bg) = if core.light_theme {
            (0xE9EBEF, 0xD9DCE2, 0xD0D4DB)
        } else {
            (0x191C21, 0x21252D, 0x1D2129)
        };
        let show_home = self.show_home;
        let tween_on = core.project.tween_playback;
        let follow_on = core.project.camera_follow.is_some();
        let paths_on = core.show_paths;
        let easing_label = format!("Easing: {}", core.project.easing.label());
        let tr = |k: &str| crate::i18n::t(core.lang, k);
        let lang_label = format!("Lang: {}", core.lang.label());
        let fease_label = core.frame_easing_label();
        let lib_name = core
            .library
            .get(core.library_index)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "Library".into());

        let data = self.draw_data();
        let bounds_slot = self.canvas_bounds.clone();
        let canvas = canvas(
            move |bounds, _window, _cx| {
                *bounds_slot.borrow_mut() = bounds;
            },
            move |bounds, _state, window, _cx| {
                paint_stage(window, bounds, &data);
            },
        )
        .flex_1()
        .w_full();

        let mut sidebar = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(6.0))
            .w(px(168.0))
            .bg(rgb(bar_bg))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9AA3B2))
                    .child(tr("figures_layers")),
            );
        for (fi, fig) in core.project.figures.iter().enumerate() {
            let selected = fi == core.selected_figure;
            let name = fig.name.clone();
            let hidden = fig.hidden;
            sidebar = sidebar.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(3.0))
                    .child(
                        div()
                            .id(("fig-sel", fi))
                            .flex_1()
                            .px(px(6.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(if selected { rgb(0x3D5A99) } else { rgb(0x2C313A) })
                            .text_size(px(12.0))
                            .text_color(rgb(0xE8E8EC))
                            .on_click(cx.listener(move |view, _ev, _w, cx| {
                                view.core.select_figure(fi);
                                cx.notify();
                            }))
                            .child(name),
                    )
                    .child(btn(("fig-up", fi), "^", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.move_figure(1);
                        cx.notify();
                    })))
                    .child(btn(("fig-dn", fi), "v", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.move_figure(-1);
                        cx.notify();
                    })))
                    .child(btn(("fig-eye", fi), if hidden { "off" } else { "eye" }, hidden, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.toggle_figure_hidden();
                        cx.notify();
                    })))
                    .child(btn(("fig-del", fi), "x", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.delete_figure();
                        cx.notify();
                    }))),
            );
        }

        let mut timeline = div().flex().flex_row().gap(px(6.0));
        for i in 0..core.frame_count() {
            let is_current = i == core.current;
            timeline = timeline.child(
                div()
                    .id(("frame", i))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .bg(if is_current { rgb(0x3D5A99) } else { rgb(0x2C313A) })
                    .hover(|s| s.bg(rgb(0x3A4150)))
                    .text_size(px(12.0))
                    .text_color(rgb(0xE8E8EC))
                    .on_click(cx.listener(move |view, _ev, _w, cx| {
                        view.core.goto(i);
                        cx.notify();
                    }))
                    .child(format!("{}", i + 1)),
            );
        }

        // Home panel: saved projects (from the library scan) with
        // an Open button each, plus the recents list. Built only
        // while shown; it replaces the canvas in the middle row.
        let home_panel = if show_home {
            let mut panel = div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(6.0))
                .p(px(12.0))
                .bg(rgb(row_bg))
                .child(
                    div()
                        .text_size(px(15.0))
                        .text_color(rgb(0xE8E8EC))
                        .child("Home — saved projects in ./projects"),
                );
            let entries = crate::projects::scan(std::path::Path::new("projects"));
            let mut listed = 0usize;
            for (i, info) in entries.iter().enumerate() {
                // The recovery snapshot parses as a project but is
                // not a library entry; the Recover button owns it.
                if info
                    .path
                    .file_name()
                    .map(|n| n == crate::shell::RECOVERY_FILE)
                    .unwrap_or(false)
                {
                    continue;
                }
                listed += 1;
                let line = format!(
                    "{} — {} figure(s), {} frame(s), {} fps",
                    info.name, info.figures, info.frames, info.fps as i32
                );
                let path = info.path.clone();
                let name = info.name.clone();
                panel = panel.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.0))
                                .text_color(rgb(0xE8E8EC))
                                .child(line),
                        )
                        .child(btn(("home-open", i), "Open", false, cx.listener(move |view, _ev, _w, cx| {
                            view.open_home_project(&path, &name);
                            cx.notify();
                        }))),
                );
            }
            if listed == 0 {
                panel = panel.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0x9AA3B2))
                        .child("No saved projects yet — use \"Save project\" to add one."),
                );
            }
            panel = panel.child(
                div()
                    .pt(px(10.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x9AA3B2))
                    .child("Recent projects:"),
            );
            let recents = crate::shell::recents();
            if recents.is_empty() {
                panel = panel.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0x9AA3B2))
                        .child("(none yet)"),
                );
            }
            for name in recents {
                panel = panel.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xE8E8EC))
                        .child(format!("- {name}")),
                );
            }
            panel = panel.child(btn("home-close", "Close", false, cx.listener(|view, _ev, _w, cx| {
                view.show_home = false;
                cx.notify();
            })));
            Some(panel)
        } else {
            None
        };

        let middle = div().flex().flex_row().flex_1().child(sidebar);
        let middle = match home_panel {
            Some(panel) => middle.child(panel),
            None => middle.child(canvas),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|view, ev: &KeyDownEvent, _w, cx| {
                let key = ev.keystroke.key.as_str();
                let ctrl = ev.keystroke.modifiers.control;
                let shift = ev.keystroke.modifiers.shift;
                match (key, ctrl, shift) {
                    ("z", true, false) => view.core.undo(),
                    ("z", true, true) | ("y", true, _) => view.core.redo(),
                    ("left", _, _) => {
                        let n = view.core.frame_count();
                        view.core.goto((view.core.current + n - 1) % n);
                    }
                    ("right", _, _) => {
                        let n = view.core.frame_count();
                        view.core.goto((view.core.current + 1) % n);
                    }
                    ("space", _, _) => {
                        view.toggle_play(cx);
                        return;
                    }
                    ("n", false, _) => view.core.add_frame(),
                    ("i", false, _) => view.core.insert_inbetween(),
                    ("o", false, _) => view.core.onion = !view.core.onion,
                    ("c", false, _) => view.core.camera_mode = !view.core.camera_mode,
                    _ => return,
                }
                cx.notify();
            }))
            .bg(rgb(root_bg))
            .on_mouse_down(MouseButton::Left, cx.listener(|view, ev: &MouseDownEvent, _w, cx| {
                let p = view.to_stage(ev.position);
                if view.core.draw_mode {
                    view.core.begin_draw(p);
                    cx.notify();
                } else if view.core.camera_mode {
                    view.core.begin_camera_drag(p);
                    cx.notify();
                } else if view.core.begin_drag(p) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|view, ev: &MouseMoveEvent, _w, cx| {
                if view.core.draw_mode {
                    let p = view.to_stage(ev.position);
                    view.core.draw_to(p);
                    cx.notify();
                } else if view.core.dragging.is_some() {
                    let p = view.to_stage(ev.position);
                    view.core.drag_to(p);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|view, _ev: &MouseUpEvent, _w, cx| {
                if view.core.draw_mode {
                    view.core.end_draw();
                    cx.notify();
                } else if view.core.dragging.is_some() {
                    view.core.end_drag();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(rgb(bar_bg))
                    .child(btn("play", if playing { tr("pause") } else { tr("play") }, playing, cx.listener(|view, _ev, _w, cx| {
                        view.toggle_play(cx);
                    })))
                    .child(btn("add-frame", tr("add_frame"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_frame();
                        cx.notify();
                    })))
                    .child(btn("dup-frame", tr("duplicate_frame"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.duplicate_frame();
                        cx.notify();
                    })))
                    .child(btn("del-frame", tr("delete_frame"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.delete_frame();
                        cx.notify();
                    })))
                    .child(btn("onion", tr("onion_skin"), onion_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.onion = !view.core.onion;
                        cx.notify();
                    })))
                    .child(btn("add-figure", tr("add_figure"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_figure();
                        cx.notify();
                    })))
                    .child(btn("undo", tr("undo"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.undo();
                        cx.notify();
                    })))
                    .child(btn("redo", tr("redo"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.redo();
                        cx.notify();
                    })))
                    .child(btn("copy-frame", tr("copy_frame"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.copy_frame();
                        cx.notify();
                    })))
                    .child(btn("paste-frame", tr("paste_frame"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.paste_frame();
                        cx.notify();
                    })))
                    .child(btn("save", tr("save"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.save();
                        cx.notify();
                    })))
                    .child(btn("load", tr("load"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.load();
                        cx.notify();
                    })))
                    .child(btn("export", tr("export_png"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_png();
                        cx.notify();
                    })))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0x9AA3B2))
                            .child(format!("{}   ·   {}", frame_label, fps_label)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(row_bg))
                    .child(btn("inbetween", tr("inbetween"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.insert_inbetween();
                        cx.notify();
                    })))
                    .child(btn("add-seg", tr("add_segment"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_segment();
                        cx.notify();
                    })))
                    .child(btn("del-seg", tr("delete_segment"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.delete_segment();
                        cx.notify();
                    })))
                    .child(btn("len-up", tr("len_plus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_length(4.0);
                        cx.notify();
                    })))
                    .child(btn("len-dn", tr("len_minus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_length(-4.0);
                        cx.notify();
                    })))
                    .child(btn("thick-up", tr("thick_plus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_thickness(1.0);
                        cx.notify();
                    })))
                    .child(btn("thick-dn", tr("thick_minus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_thickness(-1.0);
                        cx.notify();
                    })))
                    .child(btn("node-color", "Seg colour", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_node_color();
                        cx.notify();
                    })))
                    .child(btn("node-shape", tr("shape"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_shape();
                        cx.notify();
                    })))
                    .child(btn("draw-mode", tr("draw"), draw_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.draw_mode = !view.core.draw_mode;
                        view.core.status = if view.core.draw_mode {
                            "Draw mode: drag on the canvas to paint a stroke on this frame.".into()
                        } else {
                            "Pose mode.".into()
                        };
                        cx.notify();
                    })))
                    .child(btn("clear-drawing", tr("clear_drawing"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.clear_frame_drawings();
                        cx.notify();
                    })))
                    .child(btn("add-quad", "+ Quadruped", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_quadruped();
                        cx.notify();
                    })))
                    .child(btn("add-mini", "+ Mini", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_simple();
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(row_bg))
                    .child(btn("cam-mode", tr("camera_mode"), cam_mode, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_mode = !view.core.camera_mode;
                        view.core.status = if view.core.camera_mode {
                            "Camera mode: drag the canvas to pan. Keys are set per frame.".into()
                        } else {
                            "Pose mode.".into()
                        };
                        cx.notify();
                    })))
                    .child(btn("cam-zin", tr("cam_zoom_plus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_zoom_by(1.2);
                        cx.notify();
                    })))
                    .child(btn("cam-zout", tr("cam_zoom_minus"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_zoom_by(1.0 / 1.2);
                        cx.notify();
                    })))
                    .child(btn("cam-clear", "Clear cam key", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_clear_key();
                        cx.notify();
                    })))
                    .child(btn("export-gif", tr("export_gif"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_gif();
                        cx.notify();
                    })))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9AA3B2))
                            .child(cam_label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(row_bg))
                    .child(btn("fps-dn", "FPS -", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_fps(-1.0);
                        cx.notify();
                    })))
                    .child(btn("fps-up", "FPS +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_fps(1.0);
                        cx.notify();
                    })))
                    .child(btn("dur-dn", "Dur -", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_frame_duration(0.5);
                        cx.notify();
                    })))
                    .child(btn("dur-up", "Dur +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_frame_duration(2.0);
                        cx.notify();
                    })))
                    .child(btn("tween", tr("tween_play"), tween_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.toggle_tween_playback();
                        cx.notify();
                    })))
                    .child(btn("easing", Box::leak(easing_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_easing();
                        cx.notify();
                    })))
                    .child(btn("follow", tr("follow_cam"), follow_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.toggle_camera_follow();
                        cx.notify();
                    })))
                    .child(btn("paths", "Paths", paths_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.show_paths = !view.core.show_paths;
                        cx.notify();
                    })))
                    .child(btn("fx-burst", tr("fx_burst"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_particle(0);
                        cx.notify();
                    })))
                    .child(btn("fx-sparkle", tr("fx_sparkle"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_particle(1);
                        cx.notify();
                    })))
                    .child(btn("fx-smoke", tr("fx_smoke"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_particle(2);
                        cx.notify();
                    })))
                    .child(btn("fx-clear", tr("clear_fx"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.clear_particles();
                        cx.notify();
                    })))
                    .child(btn("lib-cycle", Box::leak(format!("Lib: {lib_name}").into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_library();
                        cx.notify();
                    })))
                    .child(btn("lib-insert", "Insert lib fig", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.insert_library_figure();
                        cx.notify();
                    })))
                    .child(btn("lib-save", "Save fig to lib", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.save_figure_to_library();
                        cx.notify();
                    })))
                    .child(btn("export-mp4", "Export MP4", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_mp4();
                        cx.notify();
                    })))
                    .child(btn("export-sheet", tr("spritesheet"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_sheet();
                        cx.notify();
                    })))
                    .child(btn("theme", if core.light_theme { tr("dark_theme") } else { tr("light_theme") }, false, cx.listener(|view, _ev, _w, cx| {
                        view.core.light_theme = !view.core.light_theme;
                        cx.notify();
                    })))
                    .child(btn("onion-range", "Onion +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_onion_frames();
                        cx.notify();
                    })))
                    .child(btn("import-piv", tr("import_piv"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.import_piv();
                        cx.notify();
                    })))
                    .child(btn("import-stk", tr("import_stk"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.import_stk();
                        cx.notify();
                    })))
                    .child(btn("export-piv", tr("export_piv"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_piv();
                        cx.notify();
                    })))
                    .child(btn("export-stk", tr("export_stk"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_stk();
                        cx.notify();
                    })))
                    .child(btn("add-audio", tr("add_audio"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_audio_clip();
                        cx.notify();
                    })))
                    .child(btn("make-clip", tr("make_clip"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.make_clip_from_figure();
                        cx.notify();
                    })))
                    .child(btn("insert-clip", tr("insert_clip"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.insert_clip_figure();
                        cx.notify();
                    })))
                    .child(btn("export-webm", tr("export_webm"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_webm();
                        cx.notify();
                    })))
                    .child(btn("export-svg", tr("export_svg"), false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_svg();
                        cx.notify();
                    })))
                    .child(btn("save-proj", "Save project", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.save_named_project();
                        // Keep the recovery snapshot and the recents
                        // list in step with explicit saves.
                        let _ = crate::shell::write_recovery(&view.core.project);
                        crate::shell::touch_recent(&view.core.project.name);
                        cx.notify();
                    })))
                    .child(btn("cycle-proj", "Projects >", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_project_file();
                        cx.notify();
                    })))
                    .child(btn("open-proj", "Open project", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.open_project_file();
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(rgb(bar_bg))
                    .child(btn("export-v5", "Export .piv v5", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_v5_file();
                        cx.notify();
                    })))
                    .child(btn("export-apng", "Export APNG", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_apng_file();
                        cx.notify();
                    })))
                    .child(btn("export-jpeg", "Export JPEG", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.export_jpeg_dir();
                        cx.notify();
                    })))
                    .child(btn("import-nodes", "Import .nodes", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.import_nodes_file();
                        cx.notify();
                    })))
                    .child(btn("lang", Box::leak(lang_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_lang();
                        cx.notify();
                    })))
                    .child(btn("frame-easing", Box::leak(fease_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_frame_easing();
                        cx.notify();
                    })))
                    .child(btn("fxclip-beam", "FX clip Beam", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_fx_clip(crate::vfx::ClipKind::Beam);
                        cx.notify();
                    })))
                    .child(btn("fxclip-aura", "FX clip Aura", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_fx_clip(crate::vfx::ClipKind::Aura);
                        cx.notify();
                    })))
                    .child(btn("fxclip-trail", "FX clip Trail", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_fx_clip(crate::vfx::ClipKind::Trail);
                        cx.notify();
                    })))
                    .child(btn("home", "Home", show_home, cx.listener(|view, _ev, _w, cx| {
                        view.show_home = !view.show_home;
                        if view.show_home {
                            // Snapshot the current work as the
                            // recovery file whenever Home is opened.
                            let _ = crate::shell::write_recovery(&view.core.project);
                        }
                        cx.notify();
                    })))
                    .child(btn("recover", "Recover", false, cx.listener(|view, _ev, _w, cx| {
                        match crate::shell::read_recovery() {
                            Some(project) => {
                                view.core.project = project;
                                view.core.current = 0;
                                view.core.playhead = 0.0;
                                view.core.selected_figure = 0;
                                view.core.selected_node = None;
                                view.core.status =
                                    "Recovered the snapshot from ./projects/.recovery.json.".into();
                            }
                            None => {
                                view.core.status =
                                    "No recovery snapshot found in ./projects/.".into();
                            }
                        }
                        cx.notify();
                    })))
                    .child(btn("stage-w-dn", "Stage W -", false, cx.listener(|view, _ev, _w, cx| {
                        view.adjust_stage(-40.0, 0.0);
                        cx.notify();
                    })))
                    .child(btn("stage-w-up", "Stage W +", false, cx.listener(|view, _ev, _w, cx| {
                        view.adjust_stage(40.0, 0.0);
                        cx.notify();
                    })))
                    .child(btn("stage-h-dn", "Stage H -", false, cx.listener(|view, _ev, _w, cx| {
                        view.adjust_stage(0.0, -40.0);
                        cx.notify();
                    })))
                    .child(btn("stage-h-up", "Stage H +", false, cx.listener(|view, _ev, _w, cx| {
                        view.adjust_stage(0.0, 40.0);
                        cx.notify();
                    })))
                    .child(btn("import-stknds", "Import .stknds", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.import_nodes_anim_file();
                        cx.notify();
                    })))
                    .child(btn("node-cap", "Seg cap", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_node_cap();
                        cx.notify();
                    })))
                    .child(btn("node-taper", "Taper", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_node_taper();
                        cx.notify();
                    })))
                    .child(btn("node-gradient", "Gradient", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_node_gradient();
                        cx.notify();
                    })))
                    .child(btn("node-image", "Img seg", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.attach_image_to_node();
                        cx.notify();
                    })))
                    .child(btn("node-media-clear", "Clear media", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.clear_node_media();
                        cx.notify();
                    })))
                    .child(btn("mixdown", "Mixdown", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.mixdown_audio();
                        cx.notify();
                    }))),
            )
            .child(middle)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(rgb(bar_bg))
                    .child(timeline)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9AA3B2))
                            .child(status),
                    ),
            )
    }
}

fn paint_stage(window: &mut Window, bounds: Bounds<Pixels>, data: &DrawData) {
    let origin = bounds.origin;
    let (cam_x, cam_y, zoom) = data.cam;
    let fit = (f32::from(bounds.size.width) / data.stage[0])
        .min(f32::from(bounds.size.height) / data.stage[1]);
    let s = fit * zoom;
    let cxw = f32::from(bounds.size.width) / 2.0;
    let cyw = f32::from(bounds.size.height) / 2.0;
    let map = |p: V2| -> Point<Pixels> {
        point(
            origin.x + px(cxw + (p[0] - cam_x) * s),
            origin.y + px(cyw + (p[1] - cam_y) * s),
        )
    };
    let stroke = |window: &mut Window, segs: &[(V2, V2)], color: Rgba, width: f32| {
        let mut pb = PathBuilder::stroke(px(width));
        for (a, b) in segs {
            pb.move_to(map(*a));
            pb.line_to(map(*b));
        }
        if let Ok(path) = pb.build() {
            window.paint_path(path, color);
        }
    };
    let stroke_seg = |window: &mut Window, a: V2, b: V2, color: u32, width: f32| {
        let mut pb = PathBuilder::stroke(px(width));
        pb.move_to(map(a));
        pb.line_to(map(b));
        if let Ok(path) = pb.build() {
            window.paint_path(path, rgb(color));
        }
    };
    // Onion skin (previous frame) first, translucent.
    for (segs, color, alpha) in &data.onion {
        stroke(window, segs, rgba((color << 8) | *alpha as u32), 5.0 * s);
    }
    // Brush strokes for this frame, under the figures.
    for (points, color, width) in &data.strokes {
        if points.len() > 1 {
            let segs: Vec<(V2, V2)> = points.windows(2).map(|w| (w[0], w[1])).collect();
            stroke(window, &segs, rgb(*color), width * s);
        }
    }
    // Motion paths (root trails + selected joint trail).
    for path in &data.paths {
        if path.len() > 1 {
            let segs: Vec<(V2, V2)> = path.windows(2).map(|w| (w[0], w[1])).collect();
            stroke(window, &segs, rgba(0x9AA3B2 << 8 | 0x66), 1.5 * s.max(0.5));
        }
    }
    for (fi, (segs, joints, heads, color)) in data.figures.iter().enumerate() {
        for (a, b, seg_color, thickness) in segs {
            stroke_seg(window, *a, *b, *seg_color, thickness * s);
        }
        // Head rings.
        for h in heads {
            let mut pb = PathBuilder::stroke(px(5.0 * s));
            trace_circle(&mut pb, map(*h), 17.0 * s);
            if let Ok(path) = pb.build() {
                window.paint_path(path, rgb(*color));
            }
        }
        let _ = fi;
        // Joint handles.
        for (ni, j) in joints.iter().enumerate() {
            let selected = data.selected == Some((fi, ni));
            let r = if selected { 8.0 } else { 5.0 } * s.max(0.6);
            let mut pb = PathBuilder::fill();
            trace_circle(&mut pb, map(*j), r);
            if let Ok(path) = pb.build() {
                window.paint_path(path, if selected { rgb(0xFFC65D) } else { rgb(0xFF5D5D) });
            }
        }
    }
    // Shape segments: circles and regular polygons.
    for (centre, shape, length, color, thickness) in &data.shapes {
        let r = length / 2.0 * s;
        match shape {
            crate::model::Shape::Circle => {
                let mut pb = PathBuilder::stroke(px(thickness * s));
                trace_circle(&mut pb, map(*centre), r);
                if let Ok(path) = pb.build() {
                    window.paint_path(path, rgb(*color));
                }
            }
            crate::model::Shape::Polygon(sides) => {
                let sides = (*sides).max(3) as usize;
                let mut pb = PathBuilder::stroke(px(thickness * s));
                for k in 0..=sides {
                    let a = k as f32 / sides as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                    let pt = point(
                        map(*centre).x + px(r * a.cos()),
                        map(*centre).y + px(r * a.sin()),
                    );
                    if k == 0 {
                        pb.move_to(pt);
                    } else {
                        pb.line_to(pt);
                    }
                }
                if let Ok(path) = pb.build() {
                    window.paint_path(path, rgb(*color));
                }
            }
            crate::model::Shape::Line => {}
        }
    }
    // Particles.
    for (p, r, color, alpha) in &data.particles {
        let a = (*alpha * 255.0) as u32;
        let mut pb = PathBuilder::fill();
        trace_circle(&mut pb, map(*p), (r * s).max(0.8));
        if let Ok(path) = pb.build() {
            window.paint_path(path, rgba((color << 8) | a));
        }
    }
}

/// Trace a circle as a 28-gon (GPUI 0.2.2's PathBuilder has no
/// add_circle; at handle/ring sizes this is visually exact).
fn trace_circle(pb: &mut PathBuilder, center: Point<Pixels>, radius: f32) {
    let n = 28;
    for i in 0..=n {
        let a = (i as f32 / n as f32) * std::f32::consts::TAU;
        let p = point(center.x + px(radius * a.cos()), center.y + px(radius * a.sin()));
        if i == 0 {
            pb.move_to(p);
        } else {
            pb.line_to(p);
        }
    }
}
