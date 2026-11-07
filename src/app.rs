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
}

impl EditorView {
    pub fn new() -> Self {
        EditorView {
            core: EditorCore::new(),
            canvas_bounds: Rc::new(RefCell::new(Bounds::default())),
        }
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
            cx.spawn(async move |this, cx| {
                loop {
                    let fps = match this.read_with(cx, |view: &EditorView, _| view.core.project.fps) {
                        Ok(f) => f,
                        Err(_) => break,
                    };
                    cx.background_executor()
                        .timer(Duration::from_millis((1000.0 / fps.max(1.0)) as u64))
                        .await;
                    let keep = this
                        .update(cx, |view, cx| {
                            if view.core.playing {
                                view.core.advance();
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
    onion: Vec<(Vec<(V2, V2)>, u32)>,
    selected: Option<(usize, usize)>,
    cam: (f32, f32, f32),
}

impl EditorView {
    fn draw_data(&self) -> DrawData {
        let core = &self.core;
        let mut figures = Vec::new();
        for fi in 0..core.project.figures.len() {
            let pos = core.positions(core.current, fi);
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
            figures.push((segs, pos, heads, fig_color));
        }
        let mut onion = Vec::new();
        if core.onion && core.current > 0 {
            for fi in 0..core.project.figures.len() {
                let pos = core.positions(core.current - 1, fi);
                let def = &core.project.figures[fi].def;
                let mut segs = Vec::new();
                for (i, node) in def.nodes.iter().enumerate() {
                    if let Some(p) = node.parent {
                        segs.push((pos[p], pos[i]));
                    }
                }
                onion.push((segs, core.project.figures[fi].color));
            }
        }
        DrawData {
            stage: core.project.stage,
            background: core.project.background,
            figures,
            onion,
            selected: self.core.selected_node.map(|n| (self.core.selected_figure, n)),
            cam: core.camera_here(),
        }
    }
}

fn btn(
    id: &'static str,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let core = &self.core;
        let frame_label = format!("Frame {} / {}", core.current + 1, core.frame_count());
        let fps_label = format!("{} fps", core.project.fps as i32);
        let status = core.status.clone();
        let playing = core.playing;
        let onion_on = core.onion;
        let cam_mode = core.camera_mode;
        let (ccx, ccy, czoom) = core.camera_here();
        let cam_label = format!("cam ({ccx:.0}, {ccy:.0}) x{czoom:.2} · {} key(s)", core.project.camera.len());

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

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x191C21))
            .on_mouse_down(MouseButton::Left, cx.listener(|view, ev: &MouseDownEvent, _w, cx| {
                let p = view.to_stage(ev.position);
                if view.core.camera_mode {
                    view.core.begin_camera_drag(p);
                    cx.notify();
                } else if view.core.begin_drag(p) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|view, ev: &MouseMoveEvent, _w, cx| {
                if view.core.dragging.is_some() {
                    let p = view.to_stage(ev.position);
                    view.core.drag_to(p);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|view, _ev: &MouseUpEvent, _w, cx| {
                if view.core.dragging.is_some() {
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
                    .bg(rgb(0x21252D))
                    .child(btn("play", if playing { "Pause" } else { "Play" }, playing, cx.listener(|view, _ev, _w, cx| {
                        view.toggle_play(cx);
                    })))
                    .child(btn("add-frame", "+ Frame", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_frame();
                        cx.notify();
                    })))
                    .child(btn("dup-frame", "Duplicate", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.duplicate_frame();
                        cx.notify();
                    })))
                    .child(btn("del-frame", "Delete", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.delete_frame();
                        cx.notify();
                    })))
                    .child(btn("onion", "Onion skin", onion_on, cx.listener(|view, _ev, _w, cx| {
                        view.core.onion = !view.core.onion;
                        cx.notify();
                    })))
                    .child(btn("add-figure", "+ Figure", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_figure();
                        cx.notify();
                    })))
                    .child(btn("undo", "Undo", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.undo();
                        cx.notify();
                    })))
                    .child(btn("save", "Save", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.save();
                        cx.notify();
                    })))
                    .child(btn("load", "Load", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.load();
                        cx.notify();
                    })))
                    .child(btn("export", "Export PNG", false, cx.listener(|view, _ev, _w, cx| {
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
                    .bg(rgb(0x1D2129))
                    .child(btn("inbetween", "Inbetween", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.insert_inbetween();
                        cx.notify();
                    })))
                    .child(btn("add-seg", "+ Segment", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.add_segment();
                        cx.notify();
                    })))
                    .child(btn("del-seg", "- Segment", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.delete_segment();
                        cx.notify();
                    })))
                    .child(btn("len-up", "Len +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_length(4.0);
                        cx.notify();
                    })))
                    .child(btn("len-dn", "Len -", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_length(-4.0);
                        cx.notify();
                    })))
                    .child(btn("thick-up", "Thick +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_thickness(1.0);
                        cx.notify();
                    })))
                    .child(btn("thick-dn", "Thick -", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.adjust_thickness(-1.0);
                        cx.notify();
                    })))
                    .child(btn("node-color", "Seg colour", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.cycle_node_color();
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
                    .bg(rgb(0x1D2129))
                    .child(btn("cam-mode", "Camera mode", cam_mode, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_mode = !view.core.camera_mode;
                        view.core.status = if view.core.camera_mode {
                            "Camera mode: drag the canvas to pan. Keys are set per frame.".into()
                        } else {
                            "Pose mode.".into()
                        };
                        cx.notify();
                    })))
                    .child(btn("cam-zin", "Cam zoom +", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_zoom_by(1.2);
                        cx.notify();
                    })))
                    .child(btn("cam-zout", "Cam zoom -", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_zoom_by(1.0 / 1.2);
                        cx.notify();
                    })))
                    .child(btn("cam-clear", "Clear cam key", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.camera_clear_key();
                        cx.notify();
                    })))
                    .child(btn("export-gif", "Export GIF", false, cx.listener(|view, _ev, _w, cx| {
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
            .child(canvas)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(rgb(0x21252D))
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
    for (segs, color) in &data.onion {
        stroke(window, segs, rgba((color << 8) | 0x4D), 5.0 * s);
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
