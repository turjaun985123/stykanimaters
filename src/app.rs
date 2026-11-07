//! GPUI view: toolbar, canvas (figure rendering + mouse posing) and
//! the frame timeline. All document logic lives in `editor.rs`.

use crate::editor::EditorCore;
use crate::model::V2;
#[allow(unused_imports)]
use crate::ui_kit::*;
use crate::ui_hud::{hud_strip, HudState};
use crate::touch::{Gesture, PointerEvent, PointerKind, MOUSE_POINTER_ID};
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Which channel the graph panel edits (wave 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphChannelSel {
    Angle,
    RootX,
    RootY,
}

/// Which of a graph key's two Hermite tangents a handle drag
/// edits (wave 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TanSide {
    In,
    Out,
}

impl TanSide {
    fn label(&self) -> &'static str {
        match self {
            TanSide::In => "incoming",
            TanSide::Out => "outgoing",
        }
    }
}

/// Wave 9: an in-progress motion-path point drag — the grabbed
/// frame root of the selected figure. `start` is the stage-space
/// press point; `delta` is the total stage-space drag offset
/// accumulated from it (kept current by the mouse-move handler,
/// previewed by `draw_data`, and applied once — through
/// `EditorCore::nudge_path_point` — when the drag ends).
#[derive(Clone, Copy)]
struct PathDrag {
    frame: usize,
    fi: usize,
    start: V2,
    delta: V2,
}

pub struct EditorView {
    core: EditorCore,
    canvas_bounds: Rc<RefCell<Bounds<Pixels>>>,
    focus_handle: FocusHandle,
    player: crate::audio::AudioPlayer,
    /// When true, the home panel (saved projects + recents)
    /// replaces the canvas area.
    show_home: bool,
    /// Gesture recogniser for canvas pointer input (wave 5).
    /// GPUI 0.2.2 exposes no multi-pointer events on Linux (see
    /// `touch::PointerSource`), so today only the mouse feeds it,
    /// under `touch::MOUSE_POINTER_ID` — fed by pointer id, so a
    /// future touch source needs no redesign.
    recognizer: crate::touch::Recognizer,
    /// Monotonic clock the recogniser's timestamps come from.
    clock: Instant,
    /// Wave 7: pinch→camera driver (see `PinchCamera` below).
    /// Inert in the running app until a second pointer source
    /// exists; its recogniser input contract is test-pinned in
    /// `touch.rs`.
    pinch_cam: PinchCamera,
    /// Active text-authoring session for a segment (wave 5).
    text_edit: Option<crate::text_edit::TextEdit>,
    /// Wave 6: graph-editor panel under the canvas.
    show_graph: bool,
    /// Wave 6: which channel the graph panel edits.
    graph_channel: GraphChannelSel,
    /// Wave 6: graph canvas bounds from the last prepaint; the
    /// root mouse handlers route panel presses through them.
    graph_bounds: Rc<RefCell<Bounds<Pixels>>>,
    /// Wave 6: (frame, value) of the graph key being dragged.
    graph_drag: Option<(f32, f32)>,
    /// Wave 9: motion-path point drag in progress, if any (see
    /// `PathDrag`). Started by a press on a path vertex of the
    /// selected figure; owns mouse moves until release.
    path_drag: Option<PathDrag>,
    /// Wave 9: frame of the selected graph key — set whenever a
    /// key is grabbed or added, and follows the key while it is
    /// dragged. Unlike `graph_drag` it survives mouse-up, so the
    /// key's tangent handles stay painted (and pressable) after
    /// the key drag ends.
    graph_sel: Option<f32>,
    /// Wave 9: tangent drag in progress — (key frame, side).
    graph_tan_drag: Option<(f32, TanSide)>,
    /// Wave 7: audio panel under the canvas (mutually exclusive
    /// with the graph panel — opening one closes the other).
    show_audio: bool,
    /// Wave 8: movieclip panel under the canvas (mutually
    /// exclusive with the graph and audio panels).
    show_clips: bool,
    /// Wave 8: FX timeline panel under the canvas (mutually
    /// exclusive with the graph and audio panels — opening one
    /// closes the others).
    show_fx: bool,
    /// UI components wave: toast stack fed from core.status
    /// changes in render(); last_status detects the changes.
    toasts: crate::ui_toast::ToastQueue,
    last_status: String,
    /// UI components wave: easing picker panel under the
    /// stage (independent of the other panels, like the
    /// colour panel).
    show_easing: bool,
    /// UI components wave: help & shortcuts panel under the
    /// canvas, toggled by the app bar's "?" button.
    show_help: bool,
    /// UI components wave: segment colour picker panel under
    /// the canvas (independent flag — shown alongside whichever
    /// other panel is open).
    show_color: bool,
    /// Wave 9: weather panel under the canvas (independent
    /// flag, like the colour panel).
    show_weather: bool,
    /// Wave 8: export centre panel under the canvas — every
    /// export op gathered by category with a guidance line each
    /// (the tool-group export buttons all stay where they are).
    show_export: bool,
    /// UI components wave: right-hand inspector column for the
    /// selected figure/node (mutually exclusive with the graph,
    /// audio, FX and movieclip panels).
    show_inspector: bool,
    /// Wave 9: timeline range selection as (anchor, focus),
    /// set by shift-clicking frame chips (the first shift-click
    /// anchors at the current frame); drives the Frames group's
    /// Copy/Paste/Delete range buttons. Plain chip clicks clear it.
    range_sel: Option<(usize, usize)>,
    /// Wave 7: waveform-peaks cache, keyed by (clip path, file
    /// mtime in nanoseconds since the epoch). The value is `None`
    /// for a file that failed to probe as a WAV — caching the
    /// failure too keeps repaints from re-reading broken clips.
    audio_peaks_cache: std::collections::HashMap<(String, u64), Option<AudioPeaksEntry>>,
}

/// Pinch → camera driver (Wave 7).
///
/// A pinch is a camera gesture (see `touch.rs`): the recogniser's
/// cumulative `scale` zooms and its cumulative `pan` pans. This
/// driver captures the camera at `PinchStart` and on every
/// `Pinch` writes `baseline ∘ gesture` back as the camera key at
/// the current frame, in the same terms the mouse camera drag
/// uses: `Project::set_camera_key`, centre moved opposite the
/// fingers' travel so the scene follows them, zoom clamped to
/// the 0.3..=4.0 range `EditorCore::camera_zoom_by` uses. The
/// whole pinch is a single undo step: `begin_camera_drag`
/// snapshots once at the start, and this driver writes keys
/// directly instead of calling `camera_zoom_by` per event (that
/// op snapshots on every call).
///
/// GPUI 0.2.2 cannot deliver a pinch on Linux today (see
/// `touch::PointerSource`), so in the running app this driver
/// only ever sees what a future second pointer source would
/// emit. Its input contract — the recogniser's cumulative
/// scale/pan for a scripted two-pointer stream — is pinned by
/// a test in `touch.rs`; app.rs hosts no tests because any
/// `#[test]` in this file currently blows the test profile's
/// recursion limit (the accumulated GPUI builder nesting sits
/// at the edge; raising the limit segfaults rustc in
/// gpui_macros instead — Wave 7 finding, integrator-flagged).
struct PinchCamera {
    /// Camera (x, y, zoom) captured at `PinchStart`; `None`
    /// between pinches.
    baseline: Option<(f32, f32, f32)>,
}

impl PinchCamera {
    fn new() -> Self {
        PinchCamera { baseline: None }
    }

    /// Handle one gesture; returns true when it was a pinch
    /// gesture (consumed), false for anything else.
    fn apply(&mut self, core: &mut EditorCore, gesture: Gesture) -> bool {
        match gesture {
            Gesture::PinchStart { centre } => {
                self.baseline = Some(core.camera_here());
                // Snapshots once and arms DragKind::Camera, so
                // the core's bookkeeping (and the mouse handlers'
                // mid-drag safety nets) treat the pinch exactly
                // like a camera drag.
                core.begin_camera_drag(centre);
                true
            }
            Gesture::Pinch { scale, pan, .. } => {
                if let Some((bx, by, bz)) = self.baseline {
                    let zoom = (bz * scale).clamp(0.3, 4.0);
                    core.project
                        .set_camera_key(core.current, bx - pan[0], by - pan[1], zoom);
                }
                true
            }
            Gesture::PinchEnd => {
                if self.baseline.take().is_some() {
                    let (_, _, zoom) = core.camera_here();
                    if core.dragging.is_some() {
                        core.end_drag();
                    }
                    core.status = format!("Camera zoom {zoom:.2} (pinch).");
                }
                true
            }
            _ => false,
        }
    }
}

impl EditorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        EditorView {
            core: EditorCore::new(),
            canvas_bounds: Rc::new(RefCell::new(Bounds::default())),
            focus_handle: cx.focus_handle(),
            player: crate::audio::AudioPlayer::new(),
            show_home: false,
            recognizer: crate::touch::Recognizer::new(),
            clock: Instant::now(),
            pinch_cam: PinchCamera::new(),
            text_edit: None,
            show_graph: false,
            graph_channel: GraphChannelSel::Angle,
            graph_bounds: Rc::new(RefCell::new(Bounds::default())),
            graph_drag: None,
            path_drag: None,
            graph_sel: None,
            graph_tan_drag: None,
            show_audio: false,
            show_clips: false,
            show_fx: false,
            toasts: crate::ui_toast::ToastQueue::new(),
            last_status: String::new(),
            show_easing: false,
            show_help: false,
            show_color: false,
            show_weather: false,
            show_export: false,
            show_inspector: false,
            range_sel: None,
            audio_peaks_cache: std::collections::HashMap::new(),
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

    // ---- Wave 9: motion-path point dragging --------------------

    /// Hit-test the motion-path vertices of the SELECTED figure
    /// at stage point `p`: the same points `draw_data` paints —
    /// `positions(f, fi)[0]`, the figure's root at every frame.
    /// A vertex counts as hit when it lies within 8 SCREEN px of
    /// the press: the view transform is a uniform scale, so the
    /// stage-space tolerance is `8 / scale`. Returns the drag to
    /// start (nearest hit; ties go to the earliest frame), or
    /// `None` when the press should fall through to the normal
    /// pose handling — paths hidden, playback running (pose
    /// drags refuse then too), or the selected figure hidden or
    /// locked (a locked press falls through so `begin_drag` can
    /// post its "figure is locked" status as usual).
    fn path_point_hit(&self, p: V2) -> Option<PathDrag> {
        let core = &self.core;
        if core.playing || !core.show_paths {
            return None;
        }
        let fi = core.selected_figure;
        let fig = core.project.figures.get(fi)?;
        if fig.hidden || fig.locked {
            return None;
        }
        let (scale, _cam) = self.view_transform();
        if scale <= 0.0 {
            return None;
        }
        let tol = 8.0 / scale;
        let mut best: Option<(usize, f32)> = None;
        for f in 0..core.frame_count() {
            let pt = core.positions(f, fi)[0];
            let dx = p[0] - pt[0];
            let dy = p[1] - pt[1];
            let d = (dx * dx + dy * dy).sqrt();
            if d <= tol && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((f, d));
            }
        }
        best.map(|(frame, _)| PathDrag { frame, fi, start: p, delta: [0.0, 0.0] })
    }

    /// Finish a motion-path point drag: apply the accumulated
    /// delta through `EditorCore::nudge_path_point` — ONCE, so
    /// the whole drag is a single undo step (the nudge's internal
    /// snapshot captures the pre-drag project, which is still
    /// untouched: while dragging, the movement existed only as
    /// the `draw_data` vertex preview). A press released without
    /// moving applies nothing and leaves no undo entry. This
    /// mirrors the pose drags, whose `begin_drag` snapshots once
    /// and whose `drag_to` then mutates snapshot-free.
    fn end_path_drag(&mut self) {
        if let Some(pd) = self.path_drag.take() {
            if pd.delta != [0.0, 0.0] {
                self.core.nudge_path_point(pd.frame, pd.fi, pd.delta);
            }
        }
    }

    // ---- Wave 6: graph panel -------------------------------------

    /// The channel the graph panel edits: the selector's choice,
    /// where Angle with no node selected falls back to RootX
    /// (there is no joint to key).
    fn graph_target(&self) -> crate::graph::ChannelTarget {
        use crate::graph::ChannelTarget;
        match self.graph_channel {
            GraphChannelSel::Angle => match self.core.selected_node {
                Some(ni) => ChannelTarget::Angle(ni),
                None => ChannelTarget::RootX,
            },
            GraphChannelSel::RootX => ChannelTarget::RootX,
            GraphChannelSel::RootY => ChannelTarget::RootY,
        }
    }

    /// The real channel for (selected figure, target), if it exists.
    fn graph_channel_ref(&self) -> Option<&crate::graph::Channel> {
        let target = self.graph_target();
        let fig = self.core.selected_figure;
        self.core
            .project
            .channels
            .iter()
            .find(|c| c.figure == fig && c.target == target)
    }

    /// The channel used to fit the panel view: the real channel
    /// when it exists; otherwise a stand-in so `auto_fit` frames
    /// something sensible — the full circle for angles (auto_fit
    /// forces -180..=180 for angle channels), a window around the
    /// current root value for root channels. The stand-in is only
    /// ever used for fitting, never stored.
    fn graph_fit_channel(&self) -> crate::graph::Channel {
        use crate::graph::{Channel, ChannelTarget, Key};
        let target = self.graph_target();
        if let Some(ch) = self.graph_channel_ref() {
            return ch.clone();
        }
        let fig = self.core.selected_figure;
        match target {
            ChannelTarget::Angle(_) | ChannelTarget::Length(_) => Channel {
                figure: fig,
                target,
                keys: Vec::new(),
            },
            ChannelTarget::RootX | ChannelTarget::RootY => {
                let axis = if target == ChannelTarget::RootX { 0 } else { 1 };
                let cur = self
                    .core
                    .project
                    .frames
                    .get(self.core.current)
                    .and_then(|f| f.poses.get(fig))
                    .map(|p| p.root[axis])
                    .unwrap_or(0.0);
                let last = (self.core.frame_count().max(1) - 1) as f32;
                Channel {
                    figure: fig,
                    target,
                    keys: vec![Key::flat(0.0, cur - 100.0), Key::flat(last, cur + 100.0)],
                }
            }
        }
    }

    /// The panel's current view — the same computation
    /// `paint_graph` performs, with the size taken from the last
    /// painted panel bounds.
    fn graph_view(&self) -> Option<crate::graph_panel::GraphView> {
        let b = *self.graph_bounds.borrow();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let fit = self.graph_fit_channel();
        Some(crate::graph_panel::GraphView::auto_fit(
            &[fit],
            self.core.frame_count(),
            w,
            h,
        ))
    }

    /// Window position -> panel-local pixels, when the panel is
    /// shown and the position lands inside it. With `clamp` the
    /// point is clamped into the panel instead of dropped (for
    /// drags that wander outside).
    fn graph_local(&self, pos: Point<Pixels>, clamp: bool) -> Option<V2> {
        if !self.show_graph || self.show_home {
            return None;
        }
        let b = *self.graph_bounds.borrow();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let lx = f32::from(pos.x - b.origin.x);
        let ly = f32::from(pos.y - b.origin.y);
        if clamp {
            Some([lx.clamp(0.0, w), ly.clamp(0.0, h)])
        } else if lx < 0.0 || ly < 0.0 || lx > w || ly > h {
            None
        } else {
            Some([lx, ly])
        }
    }

    /// Clamp an edited value into the range the panel shows:
    /// angles live on the -180..=180 circle.
    fn graph_clamp_value(&self, target: crate::graph::ChannelTarget, value: f32) -> f32 {
        match target {
            crate::graph::ChannelTarget::Angle(_) => value.clamp(-180.0, 180.0),
            _ => value,
        }
    }

    /// Left-press in the graph panel: grab a key near the press,
    /// or add a new key at the pressed frame/value and grab it so
    /// the same press can drag it into place. The press snapshots
    /// once, so the whole drag is a single Undo step.
    fn graph_press(&mut self, local: V2) {
        let Some(view) = self.graph_view() else { return };
        let target = self.graph_target();
        // The selected key's tangent handles get first claim on
        // the press: grabbing one starts a tangent drag (one
        // snapshot, like a key drag) instead of a key drag.
        let tan_hit: Option<(f32, TanSide)> = self.graph_sel.and_then(|sel| {
            let ch = self.graph_channel_ref()?;
            let key = ch.keys.iter().find(|k| (k.frame - sel).abs() < 0.01)?;
            let (hin, hout) = graph_tan_handles(&view, key);
            let near = |p: V2| {
                let dx = p[0] - local[0];
                let dy = p[1] - local[1];
                dx * dx + dy * dy <= 8.0 * 8.0
            };
            if near(hin) {
                Some((key.frame, TanSide::In))
            } else if near(hout) {
                Some((key.frame, TanSide::Out))
            } else {
                None
            }
        });
        if let Some((frame, side)) = tan_hit {
            self.core.snapshot();
            self.graph_tan_drag = Some((frame, side));
            self.core.status = format!(
                "Graph: drag to shape the {} tangent of the key at frame {frame:.0}.",
                side.label()
            );
            return;
        }
        let hit: Option<(f32, f32)> = self.graph_channel_ref().and_then(|ch| {
            view.hit_key(ch, local, 9.0)
                .map(|idx| (ch.keys[idx].frame, ch.keys[idx].value))
        });
        self.core.snapshot();
        if let Some((frame, value)) = hit {
            self.graph_drag = Some((frame, value));
            self.graph_sel = Some(frame);
            self.core.status = format!(
                "Graph: key at frame {frame:.0} = {value:.0} — drag to move, Ctrl+click deletes. Its round handles bend the tangents."
            );
            return;
        }
        let (frame, value) = view.to_data(local);
        let frame = frame.max(0.0);
        let value = self.graph_clamp_value(target, value);
        let fig = self.core.selected_figure;
        crate::graph::channel_mut(&mut self.core.project.channels, fig, target)
            .add_key(frame, value);
        self.graph_drag = Some((frame, value));
        self.graph_sel = Some(frame);
        self.core.status = format!(
            "Graph: added key at frame {frame:.0} = {value:.0}."
        );
    }

    /// Drag the grabbed graph key to the pointer's data position.
    fn graph_drag_to(&mut self, local: V2) {
        let Some((from, _)) = self.graph_drag else { return };
        let Some(view) = self.graph_view() else { return };
        let target = self.graph_target();
        let fig = self.core.selected_figure;
        let (frame, value) = view.to_data(local);
        let frame = frame.max(0.0);
        let value = self.graph_clamp_value(target, value);
        if let Some(ch) = self
            .core
            .project
            .channels
            .iter_mut()
            .find(|c| c.figure == fig && c.target == target)
        {
            if ch.move_key(from, frame, value) {
                self.graph_drag = Some((frame, value));
                self.graph_sel = Some(frame);
            }
        }
    }

    /// Finish a graph drag (mouse-up anywhere).
    fn graph_release(&mut self) {
        if let Some((frame, value)) = self.graph_drag.take() {
            self.core.status = format!("Graph: key at frame {frame:.0} = {value:.0}.");
        }
    }

    /// Drag a grabbed tangent handle to the pointer's data
    /// position. The pointer defines a point on the key's tangent
    /// line, so the new slope is (value delta) / (frame delta)
    /// against the key — value-units per frame, the exact unit
    /// `Channel::value_at` consumes (it scales by the segment's
    /// frame span: `m = tan * dt`). The pointer must stay on the
    /// handle's own side of the key and at least 0.25 frames away
    /// from it, so the slope stays finite and the handle can't
    /// flip through the key; the slope is clamped to the panel's
    /// full visible value span per frame. On angle channels the
    /// value delta is measured on the unwrapped (shortest-path)
    /// circle, matching `value_at`'s tangent interpretation.
    fn graph_tan_drag_to(&mut self, local: V2) {
        let Some((kframe, side)) = self.graph_tan_drag else { return };
        let Some(view) = self.graph_view() else { return };
        let target = self.graph_target();
        let fig = self.core.selected_figure;
        let (pf, pv) = view.to_data(local);
        let span = (view.val_max - view.val_min).abs().max(1.0);
        let mut applied: Option<f32> = None;
        if let Some(ch) = self
            .core
            .project
            .channels
            .iter_mut()
            .find(|c| c.figure == fig && c.target == target)
        {
            if let Some(key) = ch
                .keys
                .iter_mut()
                .find(|k| (k.frame - kframe).abs() < 0.01)
            {
                let df = pf - key.frame;
                let on_side = match side {
                    TanSide::In => df <= -0.25,
                    TanSide::Out => df >= 0.25,
                };
                if on_side {
                    let mut dv = pv - key.value;
                    if matches!(target, crate::graph::ChannelTarget::Angle(_)) {
                        dv = crate::model::norm_deg(dv);
                    }
                    let slope = (dv / df).clamp(-span, span);
                    match side {
                        TanSide::In => key.in_tan = slope,
                        TanSide::Out => key.out_tan = slope,
                    }
                    applied = Some(slope);
                }
            }
        }
        if let Some(slope) = applied {
            self.core.status = format!(
                "Graph: key frame {kframe:.0} {} tangent {slope:.2}/frame.",
                side.label()
            );
        }
    }

    /// Finish a tangent drag (mouse-up anywhere).
    fn graph_tan_release(&mut self) {
        if let Some((frame, side)) = self.graph_tan_drag.take() {
            let tan = self.graph_channel_ref().and_then(|ch| {
                ch.keys
                    .iter()
                    .find(|k| (k.frame - frame).abs() < 0.01)
                    .map(|k| match side {
                        TanSide::In => k.in_tan,
                        TanSide::Out => k.out_tan,
                    })
            });
            self.core.status = match tan {
                Some(t) => format!(
                    "Graph: key frame {frame:.0} {} tangent {t:.2}/frame.",
                    side.label()
                ),
                None => "Graph: tangent drag ended.".into(),
            };
        }
    }

    /// Delete the key near a panel position (right-click /
    /// Ctrl+click). Returns true when a key was removed.
    fn graph_delete_at(&mut self, local: V2) -> bool {
        let Some(view) = self.graph_view() else { return false };
        let target = self.graph_target();
        let fig = self.core.selected_figure;
        let frame = match self.graph_channel_ref() {
            Some(ch) => match view.hit_key(ch, local, 9.0) {
                Some(idx) => ch.keys[idx].frame,
                None => return false,
            },
            None => return false,
        };
        self.core.snapshot();
        let removed = self
            .core
            .project
            .channels
            .iter_mut()
            .find(|c| c.figure == fig && c.target == target)
            .map(|ch| ch.remove_key(frame))
            .unwrap_or(false);
        if removed {
            if let Some((f, _)) = self.graph_drag {
                if (f - frame).abs() < 0.01 {
                    self.graph_drag = None;
                }
            }
            if let Some(sel) = self.graph_sel {
                if (sel - frame).abs() < 0.01 {
                    self.graph_sel = None;
                }
            }
            if let Some((f, _)) = self.graph_tan_drag {
                if (f - frame).abs() < 0.01 {
                    self.graph_tan_drag = None;
                }
            }
            self.core.status = "Graph: key deleted.".into();
        }
        removed
    }

    /// Wave 8: FX timeline panel — one row per FX clip from
    /// `fx_edit::list` (kind, anchor figure/node, start,
    /// duration), with buttons wired to the `fx_edit` ops:
    /// shift the start one frame earlier/later, duplicate,
    /// delete. Each mutation snapshots first, so a button press
    /// is a single undo step (the same discipline the core's
    /// own audio ops follow before they write).
    /// UI components wave: the easing picker — one card per
    /// `Easing` variant with its curve drawn (see
    /// `crate::ui_easing`), each card's Apply button setting
    /// the CURRENT frame's easing via
    /// `EditorCore::set_frame_easing`, and a None button in
    /// the header clearing it back to the project default.
    /// The header reads the frame number and its current
    /// easing from the project data.
    fn easing_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let core = &self.core;
        let cur = core.current.min(core.frame_count().saturating_sub(1));
        let current = core.project.frames.get(cur).and_then(|f| f.easing);
        let current_label = match current {
            Some(e) => e.label(),
            None => "project default",
        };
        let variants = [
            crate::model::Easing::Linear,
            crate::model::Easing::Smooth,
            crate::model::Easing::EaseIn,
            crate::model::Easing::EaseOut,
        ];
        let mut cards = div().flex().flex_row().flex_wrap().gap(px(6.0));
        for (i, e) in variants.iter().enumerate() {
            let e = *e;
            let card = div()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .p(px(6.0))
                .border_1()
                .border_color(rgb(C_BORDER))
                .rounded(px(8.0))
                .child(crate::ui_easing::curve_canvas(e))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_INK))
                        .child(crate::ui_easing::easing_name(e)),
                )
                .child(btn(("ease-set", i), "Apply", current == Some(e), cx.listener(move |view, _ev, _w, cx| {
                    view.core.set_frame_easing(Some(e));
                    cx.notify();
                })));
            cards = cards.child(card);
        }
        div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Easing"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child(format!("Frame {} · {}", cur + 1, current_label)),
                    )
                    .child(div().flex_1())
                    .child(btn("ease-clear", "None", current.is_none(), cx.listener(|view, _ev, _w, cx| {
                        view.core.set_frame_easing(None);
                        cx.notify();
                    }))),
            )
            .child(cards)
    }

    fn fx_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let core = &self.core;
        let infos = crate::fx_edit::list(&core.project);
        let mut col = div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Effects"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child(format!("Effects — {} clip(s)", infos.len())),
                    ),
            );
        if infos.is_empty() {
            return col.child(crate::ui_empty::empty_line(
                "No FX clips in this project.",
            ));
        }
        for info in infos {
            let i = info.index;
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(3.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .text_color(rgb(0xE8E8EC))
                        .child(format!(
                            "{} · fig {} node {} · f{} +{}",
                            info.kind,
                            info.figure + 1,
                            info.node,
                            info.start as i64 + 1,
                            info.duration as i64
                        )),
                )
                .child(btn(("fx-earlier", i), "<", false, cx.listener(move |view, _ev, _w, cx| {
                    if i < view.core.project.fx_clips.len() {
                        let start = view.core.project.fx_clips[i].clip.start;
                        if start > 0.0 {
                            view.core.snapshot();
                            crate::fx_edit::move_clip(&mut view.core.project, i, start - 1.0);
                        }
                    }
                    cx.notify();
                })))
                .child(btn(("fx-later", i), ">", false, cx.listener(move |view, _ev, _w, cx| {
                    if i < view.core.project.fx_clips.len() {
                        let start = view.core.project.fx_clips[i].clip.start;
                        view.core.snapshot();
                        crate::fx_edit::move_clip(&mut view.core.project, i, start + 1.0);
                    }
                    cx.notify();
                })))
                .child(btn(("fx-dup", i), "Dup", false, cx.listener(move |view, _ev, _w, cx| {
                    if i < view.core.project.fx_clips.len() {
                        view.core.snapshot();
                        crate::fx_edit::duplicate_clip(&mut view.core.project, i);
                    }
                    cx.notify();
                })))
                .child(btn(("fx-del", i), "x", false, cx.listener(move |view, _ev, _w, cx| {
                    if i < view.core.project.fx_clips.len() {
                        view.core.snapshot();
                        crate::fx_edit::delete_clip(&mut view.core.project, i);
                    }
                    cx.notify();
                })));
            col = col.child(row);
            // Span bar (ui::clip_row): where this clip sits on
            // the project timeline.
            let total = core.project.frames.len().max(1) as f32;
            let colour = core
                .project
                .fx_clips
                .get(i)
                .map(|e| e.clip.color)
                .unwrap_or(0xFFC6_5D);
            col = col.child(crate::ui_clip_row::clip_row(
                crate::ui_clip_row::ClipRowData {
                    name: format!(
                        "{} · fig {} node {}",
                        info.kind,
                        info.figure + 1,
                        info.node
                    ),
                    kind_label: info.kind.to_string(),
                    colour,
                    start_frac: info.start / total,
                    end_frac: info.end / total,
                },
            ));
        }
        col
    }

    /// Wave 8: the export centre — every export op from the
    /// Camera / Export / Formats tool groups gathered in one
    /// panel, organised by category, each with a one-line note
    /// saying what it writes and where it lands (paths verified
    /// against the ops in `editor.rs`: most write under
    /// `./export/`, but the v5, APNG and JPEG ops write under
    /// `out/` — the notes say so). The `xc-` buttons call the
    /// exact same `EditorCore` ops as their tool-group
    /// counterparts; no existing button is removed or changed.
    fn export_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // One row of the centre: an `xc-` button calling the
        // shared core op, plus its muted guidance line.
        let mut xc_row = |id: &'static str,
                          label: &'static str,
                          note: &'static str,
                          op: fn(&mut EditorCore)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(3.0))
                .child(btn(id, label, false, cx.listener(move |view, _ev, _w, cx| {
                    op(&mut view.core);
                    cx.notify();
                })))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .text_color(rgb(C_MUTED))
                        .child(note),
                )
        };
        let mut col = div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Export"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child("Export centre — every format in one place"),
                    ),
            );
        col = col.child(section_label("Images"));
        col = col.child(xc_row(
            "xc-png",
            "PNG",
            "PNG image sequence — one file per frame in ./export/",
            EditorCore::export_png,
        ));
        col = col.child(xc_row(
            "xc-jpeg",
            "JPEG",
            "JPEG image sequence in out/jpeg/ (out/, not export/)",
            EditorCore::export_jpeg_dir,
        ));
        col = col.child(xc_row(
            "xc-sheet",
            "Spritesheet",
            "All frames on one sheet — ./export/spritesheet.png",
            EditorCore::export_sheet,
        ));
        col = col.child(section_label("Video & animation"));
        col = col.child(xc_row(
            "xc-gif",
            "GIF",
            "Animated GIF — ./export/animation.gif",
            EditorCore::export_gif,
        ));
        col = col.child(xc_row(
            "xc-mp4",
            "MP4",
            "MP4 video via ffmpeg, audio included — ./export/animation.mp4",
            EditorCore::export_mp4,
        ));
        col = col.child(xc_row(
            "xc-webm",
            "WebM",
            "WebM video (VP9) — ./export/animation.webm",
            EditorCore::export_webm,
        ));
        col = col.child(xc_row(
            "xc-apng",
            "APNG",
            "Animated PNG — out/animation.apng (out/, not export/)",
            EditorCore::export_apng_file,
        ));
        col = col.child(xc_row(
            "xc-svg",
            "SVG",
            "One SVG per frame in ./export/svg/",
            EditorCore::export_svg,
        ));
        col = col.child(section_label("Project formats"));
        col = col.child(xc_row(
            "xc-v2",
            ".piv v2",
            "Pivot v2 project — ./export/animation-v2.piv",
            EditorCore::export_piv2,
        ));
        col = col.child(xc_row(
            "xc-v3",
            ".piv v3",
            "Pivot v3 project — ./export/animation.piv",
            EditorCore::export_piv,
        ));
        col = col.child(xc_row(
            "xc-v4",
            ".piv v4",
            "Pivot v4 project — ./export/animation-v4.piv",
            EditorCore::export_piv4,
        ));
        col = col.child(xc_row(
            "xc-v5",
            ".piv v5",
            "Pivot v5 project — out/animation-v5.piv (out/, not export/)",
            EditorCore::export_v5_file,
        ));
        col = col.child(xc_row(
            "xc-stk",
            ".stk",
            "Selected figure only — ./export/<figure>.stk",
            EditorCore::export_stk,
        ));
        col = col.child(xc_row(
            "xc-nodes",
            ".nodes",
            "Selected figure for Stick Nodes — ./export/<figure>.nodes",
            EditorCore::export_nodes,
        ));
        col = col.child(xc_row(
            "xc-stknds",
            ".stknds",
            "Whole project for Stick Nodes — ./export/animation.stknds",
            EditorCore::export_stknds,
        ));
        col
    }

    /// UI components wave: the segment colour picker — a grid
    /// of swatches from `crate::ui_color::PICKER_PALETTE`. Each
    /// swatch writes through `EditorCore::set_node_color`, the
    /// same `NodeDef.color` the "Seg colour" cycle button steps
    /// through; the swatch matching the selected segment's
    /// current colour is highlighted.
    fn color_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let core = &self.core;
        let current: Option<u32> = core.selected_node.and_then(|ni| {
            core.project
                .figures
                .get(core.selected_figure)
                .and_then(|fig| fig.def.nodes.get(ni))
                .and_then(|node| node.color)
        });
        let mut grid = div().flex().flex_wrap().gap(px(4.0));
        for (i, &color) in crate::ui_color::PICKER_PALETTE.iter().enumerate() {
            grid = grid.child(
                crate::ui_color::swatch(color, current == Some(color))
                    .id(("swatch", i))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _ev, _w, cx| {
                        view.core.set_node_color(color);
                        cx.notify();
                    })),
            );
        }
        div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Colour"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child(if core.selected_node.is_some() {
                                "Segment colour — click a swatch"
                            } else {
                                "Select a segment to colour it"
                            }),
                    ),
            )
            .child(grid)
    }

    /// Wave 9: the weather panel — kind buttons set
    /// `Project.weather`'s kind directly (keeping the tuned
    /// amount/wind when switching kinds; enabling from off uses
    /// the cycle button's per-kind presets), and steppers adjust
    /// amount (±20) and wind (±0.5). Like the Scene group's
    /// cycle button — the only other weather control; there is
    /// no weather op in editor.rs — the listeners write
    /// `core.project.weather` directly and report through
    /// `core.status`, so nothing sits between the panel and the
    /// field. Amount clamps to the Weather doc's 0..=200 (the
    /// range emission itself clamps to); wind is stage units
    /// per second and clamps to -200..=200 (at 200 a particle
    /// crosses the 960-wide stage in under five seconds, and
    /// rain's slant is already a third of its maximum).
    fn weather_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::vfx::{Weather, WeatherKind};
        let core = &self.core;
        let cur = core.project.weather;
        let kind_name = |k: WeatherKind| match k {
            WeatherKind::Rain => "Rain",
            WeatherKind::Snow => "Snow",
            WeatherKind::Fireflies => "Fireflies",
        };
        let summary = match cur {
            None => "Off — pick a kind".to_string(),
            Some(w) => format!(
                "{} · amount {:.0} · wind {}",
                kind_name(w.kind),
                w.amount,
                w.wind
            ),
        };
        let mut kind_row = group_row().child(btn(
            "wx-none",
            "Off",
            cur.is_none(),
            cx.listener(|view, _ev, _w, cx| {
                view.core.project.weather = None;
                view.core.status = "Weather: off.".into();
                cx.notify();
            }),
        ));
        // (id, kind, preset used when enabling from off — the
        // same presets the Scene cycle button applies).
        let kinds = [
            (
                "wx-rain",
                WeatherKind::Rain,
                Weather { kind: WeatherKind::Rain, amount: 90.0, wind: 25.0 },
            ),
            (
                "wx-snow",
                WeatherKind::Snow,
                Weather { kind: WeatherKind::Snow, amount: 90.0, wind: 12.0 },
            ),
            (
                "wx-fireflies",
                WeatherKind::Fireflies,
                Weather { kind: WeatherKind::Fireflies, amount: 60.0, wind: 4.0 },
            ),
        ];
        for (id, kind, preset) in kinds {
            kind_row = kind_row.child(btn(
                id,
                kind_name(kind),
                cur.map(|w| w.kind) == Some(kind),
                cx.listener(move |view, _ev, _w, cx| {
                    view.core.project.weather = Some(match view.core.project.weather {
                        Some(mut w) => {
                            w.kind = kind;
                            w
                        }
                        None => preset,
                    });
                    view.core.status = match kind {
                        WeatherKind::Rain => "Weather: rain.",
                        WeatherKind::Snow => "Weather: snow.",
                        WeatherKind::Fireflies => "Weather: fireflies.",
                    }
                    .into();
                    cx.notify();
                }),
            ));
        }
        // One stepper row shape for both fields: label, −,
        // value, +. `step` is the field's backing helper and
        // `dn_delta` its (negative) down step; up applies the
        // negation.
        let stepper_row = |label: &'static str,
                           value: String,
                           dn: &'static str,
                           up: &'static str,
                           dn_delta: f32,
                           step: fn(&mut EditorView, f32),
                           cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_MUTED))
                        .child(label),
                )
                .child(btn(
                    dn,
                    "−",
                    false,
                    cx.listener(move |view, _ev, _w, cx| {
                        step(view, dn_delta);
                        cx.notify();
                    }),
                ))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_INK))
                        .child(value),
                )
                .child(btn(
                    up,
                    "+",
                    false,
                    cx.listener(move |view, _ev, _w, cx| {
                        step(view, -dn_delta);
                        cx.notify();
                    }),
                ))
        };
        let amount_row = stepper_row(
            "Amount",
            match cur {
                Some(w) => format!("{:.0}", w.amount),
                None => "—".to_string(),
            },
            "wx-amt-dn",
            "wx-amt-up",
            -20.0,
            Self::weather_amount_step,
            cx,
        );
        let wind_row = stepper_row(
            "Wind",
            match cur {
                Some(w) => format!("{}", w.wind),
                None => "—".to_string(),
            },
            "wx-wind-dn",
            "wx-wind-up",
            -0.5,
            Self::weather_wind_step,
            cx,
        );
        crate::ui_panel::panel_shell(
            crate::ui_panel::PanelSpec::new("Weather").with_subtitle(summary),
            div()
                .flex_col()
                .gap(px(6.0))
                .child(kind_row)
                .child(amount_row)
                .child(wind_row),
        )
    }

    /// Weather panel stepper backing: adjust the live
    /// weather's amount by `delta`, clamped to the Weather
    /// doc's 0..=200 — the same direct-write + status pattern
    /// as the Scene cycle button. Off weather just reports.
    fn weather_amount_step(view: &mut EditorView, delta: f32) {
        match view.core.project.weather {
            Some(mut w) => {
                w.amount = (w.amount + delta).clamp(0.0, 200.0);
                view.core.project.weather = Some(w);
                view.core.status = format!("Weather amount: {:.0}.", w.amount);
            }
            None => view.core.status = "Weather is off — pick a kind first.".into(),
        }
    }

    /// Weather panel stepper backing: adjust wind (stage
    /// units per second) by `delta`, clamped to -200..=200
    /// (see `weather_panel` for the range's reasoning).
    fn weather_wind_step(view: &mut EditorView, delta: f32) {
        match view.core.project.weather {
            Some(mut w) => {
                w.wind = (w.wind + delta).clamp(-200.0, 200.0);
                view.core.project.weather = Some(w);
                view.core.status = format!("Weather wind: {}.", w.wind);
            }
            None => view.core.status = "Weather is off — pick a kind first.".into(),
        }
    }

    /// Wave 7: the audio panel — one row per clip in the project:
    /// file name + start frame, a waveform-peaks strip (or a
    /// "not a readable WAV" note), and the wave-6 editor ops for
    /// that clip as buttons. Peaks come from `audio_peaks_cache`,
    /// which `render` fills for any clip whose (path, mtime) key
    /// isn't cached yet, so this method only reads.
    fn audio_panel(&self, cx: &mut Context<Self>, row_bg: u32) -> impl IntoElement {
        let core = &self.core;
        let clips = &core.project.audio;
        let mut col = div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Audio"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child(format!("Audio — {} clip(s)", clips.len())),
                    ),
            );
        if clips.is_empty() {
            return col.child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x9AA3B2))
                    .child("No audio clips in this project."),
            );
        }
        let fps = core.project.fps;
        let current = core.current as i64;
        for (i, clip) in clips.iter().enumerate() {
            let name = std::path::Path::new(&clip.path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&clip.path)
                .to_string();
            let entry = self
                .audio_peaks_cache
                .get(&(clip.path.clone(), audio_file_stamp(&clip.path)))
                .and_then(|e| e.as_ref());
            let mut row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(3.0))
                .child(
                    div()
                        .w(px(180.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0xE8E8EC))
                        .child(format!("{} · f{}", name, clip.start_frame + 1)),
                );
            match entry {
                Some(entry) => {
                    // Playhead on the first clip's strip: the
                    // current frame mapped into the clip's span
                    // (start .. start + duration in frames).
                    let playhead = if i == 0 {
                        let dur_frames = (entry.duration_secs * fps).round() as i64;
                        let start = clip.start_frame as i64;
                        if dur_frames > 0 && current >= start && current < start + dur_frames {
                            Some((current - start) as f32 / dur_frames as f32)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let strip = AudioStripData {
                        peaks: entry.peaks.clone(),
                        playhead,
                        bg: row_bg,
                    };
                    row = row.child(
                        canvas(
                            move |_bounds, _window, _cx| {},
                            move |bounds, _state, window, _cx| {
                                paint_audio_peaks(window, bounds, &strip);
                            },
                        )
                        .h(px(26.0))
                        .flex_1(),
                    );
                }
                None => {
                    row = row.child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9AA3B2))
                            .child("(not a readable WAV)"),
                    );
                }
            }
            row = row
                .child(btn(("audio-norm", i), "Norm", false, cx.listener(move |view, _ev, _w, cx| {
                    view.core.audio_normalize(i);
                    cx.notify();
                })))
                .child(btn(("audio-split", i), "Split@cur", false, cx.listener(move |view, _ev, _w, cx| {
                    let at = view.core.current;
                    view.core.audio_split(i, at);
                    cx.notify();
                })));
            if i + 1 < clips.len() {
                row = row.child(btn(("audio-xfade", i), "Xfade", false, cx.listener(move |view, _ev, _w, cx| {
                    view.core.audio_crossfade(i);
                    cx.notify();
                })));
            }
            col = col.child(row);
        }
        col
    }

    /// Wave 8: the movieclip panel — one row per clip in the
    /// project (name, frame count, used-by figure) with an Edit
    /// button that opens the wave-7 clip session on it. While a
    /// session is active, a session row offers the session's own
    /// ops: frame navigation, add/delete frame, and Exit. All
    /// edits land on `project.clips` via `EditorCore`'s session
    /// methods; this method only reads state and routes clicks.
    fn clips_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let core = &self.core;
        let clips = &core.project.clips;
        let session = core.clip_edit;
        let mut col = div()
            .flex_col()
            .w_full()
            .bg(rgb(C_PANEL))
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                top_bar()
                    .child(section_label("Movieclips"))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(C_MUTED))
                            .child(format!("Movieclips — {} clip(s)", clips.len())),
                    ),
            );
        if clips.is_empty() {
            return col.child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x9AA3B2))
                    .child("No movieclips in this project. \"Make clip\" builds one from the selected figure."),
            );
        }
        for (i, clip) in clips.iter().enumerate() {
            let used_by: Vec<&str> = core
                .project
                .figures
                .iter()
                .filter(|f| f.clip == Some(i))
                .map(|f| f.name.as_str())
                .collect();
            let usage = if used_by.is_empty() {
                "not used by any figure".to_string()
            } else {
                format!("used by {}", used_by.join(", "))
            };
            let editing = session.map(|s| s.clip) == Some(i);
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .child(
                        div()
                            .w(px(180.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0xE8E8EC))
                            .child(clip.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9AA3B2))
                            .child(format!("{} frame(s) · {}", clip.frames.len(), usage)),
                    )
                    .child(btn(("clip-edit", i), "Edit", editing, cx.listener(move |view, _ev, _w, cx| {
                        view.core.enter_clip(i);
                        cx.notify();
                    }))),
            );
        }
        if let Some(s) = session {
            let label = match clips.get(s.clip) {
                Some(clip) => format!(
                    "Editing: {} — frame {} / {}",
                    clip.name,
                    s.frame + 1,
                    clip.frames.len()
                ),
                None => "Editing session (clip missing)".to_string(),
            };
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(rgb(0xE8E8EC))
                            .child(label),
                    )
                    .child(btn("clip-prev", "< Frame", false, cx.listener(|view, _ev, _w, cx| {
                        if let Some(s) = view.core.clip_edit {
                            view.core.clip_goto(s.frame.saturating_sub(1));
                        }
                        cx.notify();
                    })))
                    .child(btn("clip-next", "Frame >", false, cx.listener(|view, _ev, _w, cx| {
                        if let Some(s) = view.core.clip_edit {
                            view.core.clip_goto(s.frame + 1);
                        }
                        cx.notify();
                    })))
                    .child(btn("clip-add-frame", "+ Frame", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.clip_add_frame();
                        cx.notify();
                    })))
                    .child(btn("clip-del-frame", "Delete frame", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.clip_delete_frame();
                        cx.notify();
                    })))
                    .child(btn("clip-exit", "Exit", false, cx.listener(|view, _ev, _w, cx| {
                        view.core.exit_clip();
                        cx.notify();
                    }))),
            );
        }
        col
    }

    /// Milliseconds on the view's monotonic clock — the epoch
    /// the gesture recogniser's events and ticks share.
    fn now_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }

    /// Apply one recognised gesture to the editor core. Dragging
    /// drives the same begin/drag/end handlers the mouse path
    /// used before the recogniser was wired in; `begin_drag`
    /// itself refuses to start while playing, and the tap /
    /// long-press selection below mirrors that refusal.
    fn apply_gesture(&mut self, gesture: Gesture) {
        // Pinch gestures drive the camera (PinchCamera). They
        // cannot arise from GPUI mouse events today — the mouse
        // is a single pointer — but the recogniser is fed by
        // pointer id, so this arm is live for any future source.
        if self.pinch_cam.apply(&mut self.core, gesture) {
            return;
        }
        match gesture {
            Gesture::DragStart { pos } => {
                self.core.begin_drag(pos);
            }
            Gesture::Drag { pos, .. } => {
                // drag_to takes the absolute position; the
                // gesture's delta is for relative-motion users.
                self.core.drag_to(pos);
            }
            Gesture::DragEnd => {
                if self.core.dragging.is_some() {
                    self.core.end_drag();
                }
            }
            Gesture::Tap { pos } => {
                if !self.core.playing {
                    if let Some((fi, ni)) = self.core.hit_test(pos) {
                        self.core.selected_figure = fi;
                        self.core.selected_node = Some(ni);
                    }
                }
            }
            Gesture::LongPress { pos } => {
                // Context action v1: select the figure and say so.
                if !self.core.playing {
                    if let Some((fi, ni)) = self.core.hit_test(pos) {
                        self.core.selected_figure = fi;
                        self.core.selected_node = Some(ni);
                        let name = self.core.project.figures[fi].name.clone();
                        self.core.status = format!("{name} selected (long-press).");
                    }
                }
            }
            // Pinch gestures are consumed by PinchCamera above;
            // this arm only exists for exhaustiveness.
            Gesture::PinchStart { .. } | Gesture::Pinch { .. } | Gesture::PinchEnd => {}
        }
    }

    fn apply_gestures(&mut self, gestures: Vec<Gesture>) {
        for gesture in gestures {
            self.apply_gesture(gesture);
        }
    }

    /// Run the recogniser's clock — the only source of
    /// `LongPress` — and apply whatever it emits. Returns
    /// whether any gesture was applied.
    fn tick_gestures(&mut self) -> bool {
        let now = self.now_ms();
        let gestures = self.recognizer.on_tick(now);
        let applied = !gestures.is_empty();
        self.apply_gestures(gestures);
        applied
    }

    /// Feed one canvas pointer event (stage coordinates) into
    /// the recogniser and apply the resulting gestures. `id`
    /// distinguishes simultaneous pointers: the GPUI mouse
    /// handlers pass `touch::MOUSE_POINTER_ID`, and a future
    /// multi-pointer source would pass each contact's own id
    /// (see `touch::PointerSource` for why GPUI 0.2.2 provides
    /// none today). Events enter through `touch::feed`, the
    /// single adapter every pointer source shares. Elapsed
    /// time is checked first, so a
    /// long-press that matured since the previous event fires
    /// before this event is processed. Returns whether any
    /// gesture was applied, so callers can skip needless
    /// repaints.
    fn feed_pointer(&mut self, id: u32, kind: PointerKind, pos: V2) -> bool {
        let ticked = self.tick_gestures();
        let now = self.now_ms();
        let gestures = crate::touch::feed(
            &mut self.recognizer,
            crate::touch::ACTIVE_POINTER_SOURCE,
            PointerEvent { id, pos, kind, t_ms: now },
        );
        let applied = !gestures.is_empty();
        self.apply_gestures(gestures);
        ticked || applied
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
                                // Keep the gesture clock running
                                // so a held pointer can long-press
                                // during playback.
                                view.tick_gestures();
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
/// Everything `paint_graph` needs, cloned out of the project at
/// render time (a channel is small; the paint closure owns it).
struct GraphDrawData {
    /// Channel used to fit the view (the real channel, or the
    /// stand-in from `EditorView::graph_fit_channel`).
    fit: crate::graph::Channel,
    /// The real channel for the selected target, if one exists.
    channel: Option<crate::graph::Channel>,
    frames: usize,
    current: f32,
    /// Frame of the key being dragged (highlighted), if any.
    drag_frame: Option<f32>,
    /// Frame of the selected key, whose tangent handles are
    /// drawn, if any.
    sel_frame: Option<f32>,
    bg: u32,
}

/// One probed WAV for the audio panel: 96 peak buckets from
/// `audio_edit::peaks`, plus the clip's duration in seconds (the
/// panel turns it into a frame span with the project's fps).
struct AudioPeaksEntry {
    peaks: Vec<f32>,
    duration_secs: f32,
}

/// Everything `paint_audio_peaks` needs for one clip row, cloned
/// out of the cache at render time (the paint closure owns it).
struct AudioStripData {
    peaks: Vec<f32>,
    /// Playhead position as a fraction of the strip width, when
    /// the editor's current frame falls inside the clip's span.
    playhead: Option<f32>,
    bg: u32,
}

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
        let visible = crate::layers::visible_figures(&core.project, core.solo);
        for fi in 0..core.project.figures.len() {
            if !visible[fi] {
                figures.push((Vec::new(), Vec::new(), Vec::new(), core.project.figures[fi].color));
                continue;
            }
            let pos = core.positions_f(display, fi);
            let def = &core.project.figures[fi].def;
            let mut segs = Vec::new();
            let mut heads = Vec::new();
            // Canvas fade for layer opacity (the rasterizer uses
            // true alpha for segments; the canvas blends to bg).
            let fig_color = crate::style::lerp_color(
                core.project.background,
                core.project.figures[fi].color,
                core.project.figures[fi].opacity.clamp(0.05, 1.0),
            );
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
                let mut poly: Vec<V2> = (0..core.frame_count())
                    .map(|f| core.positions(f, fi)[0])
                    .collect();
                // Wave 9: while a path point is being dragged,
                // preview its vertex at the dragged position.
                // The project itself is only mutated once — by
                // `nudge_path_point` when the drag ends — so the
                // polyline is the drag's live feedback.
                if let Some(pd) = &self.path_drag {
                    if pd.fi == fi {
                        if let Some(pt) = poly.get_mut(pd.frame) {
                            pt[0] += pd.delta[0];
                            pt[1] += pd.delta[1];
                        }
                    }
                }
                paths.push(poly);
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
        if let Some(w) = &core.project.weather {
            for (p, r, c, a) in crate::vfx::weather_particles(w, display, core.project.stage, core.project.fps) {
                particles.push((p, r, ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32, a));
            }
        }
        // 3D session preview: the projected figure rides along as an
        // extra cyan "figure" so the canvas shows what Bake will make.
        if let Some(st) = &core.fig3 {
            let baked = crate::fig3d::bake_nodes(&st.figure, &st.pose, &st.camera, core.project.stage);
            let mut segs = Vec::new();
            let mut joints = Vec::new();
            for (i, bn) in baked.iter().enumerate() {
                joints.push(bn.xy);
                if let Some(p) = st.figure.nodes[i].parent {
                    segs.push((baked[p].xy, bn.xy, 0x35C4D8, bn.thickness));
                }
            }
            figures.push((segs, joints, Vec::new(), 0x35C4D8));
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
        // Toast feed: a changed core.status becomes a toast (kind
        // classified from the message), then the queue ages by one
        // tick per render (~33 ms during playback).
        if self.core.status != self.last_status {
            if !self.core.status.is_empty() {
                let lower = self.core.status.to_lowercase();
                let kind = if lower.contains("failed")
                    || lower.contains("could not")
                    || lower.contains("error")
                {
                    crate::ui_toast::ToastKind::Error
                } else if lower.starts_with("exported")
                    || lower.starts_with("saved")
                    || lower.starts_with("imported")
                {
                    crate::ui_toast::ToastKind::Success
                } else {
                    crate::ui_toast::ToastKind::Info
                };
                self.toasts.push(kind, self.core.status.clone());
            }
            self.last_status = self.core.status.clone();
        }
        self.toasts.tick();
        // Autosave: writes the recovery file every 25 undoable
        // edits; silent unless the write fails.
        self.core.maybe_autosave();
        let core = &self.core;
        let frame_label = format!("Frame {} / {}", core.current + 1, core.frame_count());
        let fps_label = format!("{} fps", core.project.fps as i32);
        let status = core.status.clone();
        let status_el = if self.toasts.is_empty() {
            div()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child(status.clone())
        } else {
            crate::ui_toast::toast_stack(&self.toasts)
        };
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
        let graph_on = self.show_graph;
        let audio_on = self.show_audio;
        let clips_on = self.show_clips;
        let fx_on = self.show_fx;
        let easing_on = self.show_easing;
        let help_on = self.show_help;
        let color_on = self.show_color;
        let weather_on = self.show_weather;
        let export_on = self.show_export;
        let inspector_on = self.show_inspector;
        let tween_on = core.project.tween_playback;
        let follow_on = core.project.camera_follow.is_some();
        let paths_on = core.show_paths;
        let easing_label = format!("Easing: {}", core.project.easing.label());
        let tr = |k: &str| crate::i18n::t(core.lang, k);
        let lang_label = format!("Lang: {}", core.lang.label());
        let fps_preset_label = format!("FPS: {}", core.project.fps as i32);
        let fease_label = core.frame_easing_label();
        let lib_name = core
            .library
            .get(core.library_index)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "Library".into());

        let data = self.draw_data();
        // Wave 6: graph panel (header + canvas) that sits under
        // the stage while `show_graph` is on.
        let graph_panel = if graph_on {
            let target = self.graph_target();
            let gdata = GraphDrawData {
                fit: self.graph_fit_channel(),
                channel: self.graph_channel_ref().cloned(),
                frames: core.frame_count(),
                current: core.current as f32,
                drag_frame: self.graph_drag.map(|(f, _)| f),
                sel_frame: self.graph_sel,
                bg: row_bg,
            };
            let gslot = self.graph_bounds.clone();
            let gcanvas = canvas(
                move |bounds, _window, _cx| {
                    *gslot.borrow_mut() = bounds;
                },
                move |bounds, _state, window, _cx| {
                    paint_graph(window, bounds, &gdata);
                },
            )
            .flex_1()
            .w_full();
            let target_label = match target {
                crate::graph::ChannelTarget::Angle(ni) => format!("joint {ni} angle"),
                crate::graph::ChannelTarget::RootX => "root X".to_string(),
                crate::graph::ChannelTarget::RootY => "root Y".to_string(),
                crate::graph::ChannelTarget::Length(ni) => format!("joint {ni} length"),
            };
            let header = top_bar()
                .child(section_label("Graph"))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_MUTED))
                        .child(format!(
                            "Graph — figure {} · {}",
                            core.selected_figure + 1,
                            target_label
                        )),
                )
                .child(div().flex_1())
                .child(
                    group_row()
                        .child(btn("graph-ch-angle", "Angle", self.graph_channel == GraphChannelSel::Angle, cx.listener(|view, _ev, _w, cx| {
                            view.graph_channel = GraphChannelSel::Angle;
                            cx.notify();
                        })))
                        .child(btn("graph-ch-rootx", "RootX", self.graph_channel == GraphChannelSel::RootX, cx.listener(|view, _ev, _w, cx| {
                            view.graph_channel = GraphChannelSel::RootX;
                            cx.notify();
                        })))
                        .child(btn("graph-ch-rooty", "RootY", self.graph_channel == GraphChannelSel::RootY, cx.listener(|view, _ev, _w, cx| {
                            view.graph_channel = GraphChannelSel::RootY;
                            cx.notify();
                        }))),
                );
            Some(
                div()
                    .flex_col()
                    .h(px(170.0))
                    .w_full()
                    .bg(rgb(C_PANEL))
                    .border_1()
                    .border_color(rgb(C_BORDER))
                    .rounded(px(8.0))
                    .p(px(8.0))
                    .gap(px(6.0))
                    .child(header)
                    .child(gcanvas),
            )
        } else {
            None
        };

        // Wave 7: audio panel under the stage, mutually exclusive
        // with the graph panel (the toggles enforce that). Fill
        // the peaks cache first: any clip whose (path, mtime) key
        // is missing gets probed once — failures cache as `None`,
        // so a broken clip is not re-read on every repaint.
        let audio_panel = if audio_on {
            for clip in &core.project.audio {
                let key = (clip.path.clone(), audio_file_stamp(&clip.path));
                if !self.audio_peaks_cache.contains_key(&key) {
                    let entry = probe_audio_peaks(&clip.path);
                    self.audio_peaks_cache.insert(key, entry);
                }
            }
            Some(self.audio_panel(cx, row_bg))
        } else {
            None
        };
        let clips_panel = if clips_on {
            Some(self.clips_panel(cx))
        } else {
            None
        };

        // Wave 8: FX timeline panel under the stage, mutually
        // exclusive with the graph and audio panels.
        let fx_panel = if fx_on {
            Some(self.fx_panel(cx))
        } else {
            None
        };

        // UI components wave: easing picker panel under the
        // stage.
        let easing_panel = if easing_on {
            Some(self.easing_panel(cx))
        } else {
            None
        };

        // UI components wave: help & shortcuts panel under the
        // stage, built from the verified data in `ui_help`.
        let help_panel = if help_on {
            let shortcuts = crate::ui_help::shortcut_rows();
            let groups = crate::ui_help::group_rows();
            Some(crate::ui_help::help_panel(&shortcuts, &groups))
        } else {
            None
        };

        // UI components wave: colour picker panel under the
        // stage (independent of the other panels).
        let color_panel = if color_on {
            Some(self.color_panel(cx))
        } else {
            None
        };

        // Wave 9: weather panel under the stage (independent
        // of the other panels, like the colour panel).
        let weather_panel = if weather_on {
            Some(self.weather_panel(cx))
        } else {
            None
        };

        // Wave 8: export centre panel under the stage.
        let export_panel = if export_on {
            Some(self.export_panel(cx))
        } else {
            None
        };

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

        let mut sidebar = side_panel()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .w(px(168.0))
            .child(
                // Sidebar header: caption + bulk visibility controls.
                // The layers ops own the `hidden` flags; solo is the
                // caller's runtime state, so each listener clears it
                // to make the bulk result the effective visibility.
                top_bar()
                    .flex_wrap()
                    .child(section_label("Figures"))
                    .child(div().flex_1())
                    .child(btn("fig-vis-all", "All", false, cx.listener(|view, _ev, _w, cx| {
                        let changed = crate::layers::set_all_visible(&mut view.core.project, true);
                        view.core.solo = None;
                        view.core.status = format!("All figures shown ({changed} changed).");
                        cx.notify();
                    })))
                    .child(btn("fig-vis-none", "None", false, cx.listener(|view, _ev, _w, cx| {
                        let changed = crate::layers::set_all_visible(&mut view.core.project, false);
                        view.core.solo = None;
                        view.core.status = format!("All figures hidden ({changed} changed).");
                        cx.notify();
                    })))
                    .child(btn("fig-vis-inv", "Invert", false, cx.listener(|view, _ev, _w, cx| {
                        let flipped = crate::layers::invert_visibility(&mut view.core.project);
                        view.core.solo = None;
                        view.core.status = format!("Figure visibility inverted ({flipped} flipped).");
                        cx.notify();
                    }))),
            );
        for (fi, fig) in core.project.figures.iter().enumerate() {
            let selected = fi == core.selected_figure;
            let name = fig.name.clone();
            let hidden = fig.hidden;
            let solo = core.solo == Some(fi);
            sidebar = sidebar.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(6.0))
                    .rounded(px(6.0))
                    .bg(if selected { rgb(0x3D5A99) } else { rgb(row_bg) })
                    .child(
                        div()
                            .w(px(10.0))
                            .h(px(10.0))
                            .rounded(px(999.0))
                            .bg(rgb(fig.color)),
                    )
                    .child(
                        div()
                            .id(("fig-sel", fi))
                            .flex_1()
                            .px(px(6.0))
                            .py(px(4.0))
                            .cursor_pointer()
                            .text_size(px(13.0))
                            .text_color(rgb(0xE8E8EC))
                            .on_click(cx.listener(move |view, _ev, _w, cx| {
                                view.core.select_figure(fi);
                                cx.notify();
                            }))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(C_MUTED))
                            .child(format!("{:.0}%", fig.opacity * 100.0)),
                    )
                    .child(btn(("fig-up", fi), "^", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.move_figure_full(1);
                        cx.notify();
                    })))
                    .child(btn(("fig-dn", fi), "v", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.select_figure(fi);
                        view.core.move_figure_full(-1);
                        cx.notify();
                    })))
                    .child(btn(("fig-lock", fi), if fig.locked { "lock" } else { "open" }, fig.locked, cx.listener(move |view, _ev, _w, cx| {
                        view.core.toggle_figure_lock(fi);
                        cx.notify();
                    })))
                    .child(btn(("fig-solo", fi), "solo", solo, cx.listener(move |view, _ev, _w, cx| {
                        view.core.toggle_solo(fi);
                        cx.notify();
                    })))
                    .child(btn(("fig-opac", fi), "opac", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.cycle_figure_opacity(fi);
                        cx.notify();
                    })))
                    .child(btn(("fig-op-dn", fi), "−", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.step_figure_opacity(fi, -0.1);
                        cx.notify();
                    })))
                    .child(btn(("fig-op-up", fi), "+", false, cx.listener(move |view, _ev, _w, cx| {
                        view.core.step_figure_opacity(fi, 0.1);
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

        let range_sel = self.range_sel;
        let mut timeline = div().flex().flex_row().gap(px(4.0));
        for i in 0..core.frame_count() {
            let is_current = i == core.current;
            // In-range chips (anchor..=focus, either direction)
            // get a dimmed accent so the selection reads as a
            // block under the brighter current-frame chip.
            let in_range = match range_sel {
                Some((a, f)) => {
                    let (lo, hi) = if a <= f { (a, f) } else { (f, a) };
                    i >= lo && i <= hi
                }
                None => false,
            };
            timeline = timeline.child(
                div()
                    .id(("frame", i))
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(px(26.0))
                    .px(px(6.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .bg(if is_current {
                        rgb(C_ACCENT)
                    } else if in_range {
                        rgb(0x2E4470)
                    } else {
                        rgb(0x2C313A)
                    })
                    .hover(|s| s.bg(rgb(C_BORDER)))
                    .text_size(px(12.0))
                    .text_color(rgb(C_INK))
                    .on_click(cx.listener(move |view, ev: &ClickEvent, _w, cx| {
                        if ev.modifiers().shift {
                            // Shift-click builds the range: the
                            // first one anchors at the current
                            // frame, later ones just move focus.
                            view.range_sel = Some(match view.range_sel {
                                Some((anchor, _)) => (anchor, i),
                                None => (view.core.current, i),
                            });
                        } else {
                            view.range_sel = None;
                            view.core.goto(i);
                        }
                        cx.notify();
                    }))
                    .child(format!("{}", i + 1)),
            );
        }

        // Dopesheet strip for the bottom bar: one row per figure
        // with a dot on every frame where that figure's pose changes.
        let dope_names: Vec<String> =
            core.project.figures.iter().map(|f| f.name.clone()).collect();
        let dope_map = crate::ui_dope::keyframe_map(&core.project);
        let dopesheet = crate::ui_dope::dopesheet(&dope_map, core.current, &dope_names);

        // Timeline ruler + scrubber (ui components): frame numbers,
        // loop shading and keyframe dots aligned with the chips,
        // over a slim playhead track.
        let key_union: Vec<bool> = (0..core.project.frames.len())
            .map(|f| {
                dope_map
                    .iter()
                    .any(|row| row.get(f).copied().unwrap_or(false))
            })
            .collect();
        let ruler = crate::ui_timeline_ruler::ruler(
            core.project.frames.len(),
            core.current,
            core.loop_range,
            &key_union,
        );
        let key_fracs: Vec<f32> = if core.project.frames.len() > 1 {
            key_union
                .iter()
                .enumerate()
                .filter(|(_, k)| **k)
                .map(|(f, _)| f as f32 / (core.project.frames.len() - 1) as f32)
                .collect()
        } else {
            Vec::new()
        };
        let scrubber = crate::ui_scrubber::scrubber(
            core.playhead,
            core.project.frames.len(),
            &key_fracs,
        );

        // Home panel: saved projects (from the library scan) with
        // an Open button each, plus the recents list. Built only
        // while shown; it replaces the canvas in the middle row.
        let home_panel = if show_home {
            let mut panel = div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(10.0))
                .p(px(16.0))
                .bg(rgb(C_PANEL))
                .border_1()
                .border_color(rgb(C_BORDER))
                .rounded(px(12.0))
                .child(
                    top_bar()
                        .child(brand())
                        .child(div().flex_1())
                        .child(btn("home-close", "Close", false, cx.listener(|view, _ev, _w, cx| {
                            view.show_home = false;
                            cx.notify();
                        }))),
                )
                .child(section_label("Home — saved projects in ./projects"));
            panel = panel.child(crate::ui_stat_grid::stat_grid(
                &crate::stats::project_stats(&core.project),
            ));
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
                    "{} figure(s), {} frame(s), {} fps",
                    info.figures, info.frames, info.fps as i32
                );
                let path = info.path.clone();
                let name = info.name.clone();
                panel = panel.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .rounded(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(13.0))
                                .text_color(rgb(C_INK))
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(C_MUTED))
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
                        .text_size(px(12.0))
                        .text_color(rgb(C_MUTED))
                        .child("No saved projects yet — use \"Save project\" to add one."),
                );
            }
            panel = panel.child(section_label("Recent projects"));
            let recents = crate::shell::recents();
            if recents.is_empty() {
                panel = panel.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_MUTED))
                        .child("(none yet)"),
                );
            }
            for name in recents {
                panel = panel.child(
                    div()
                        .px(px(8.0))
                        .text_size(px(13.0))
                        .text_color(rgb(C_INK))
                        .child(format!("- {name}")),
                );
            }
            Some(panel)
        } else {
            None
        };

        // Inspector: assemble a plain data snapshot here, where the
        // editor state and evaluated pose are available. The
        // component itself only renders that snapshot; the stepper
        // controls are appended below because they need listeners.
        let inspector_panel = if inspector_on {
            let inspector_data = match core.project.figures.get(core.selected_figure) {
                Some(figure) => {
                    let selected = core
                        .selected_node
                        .and_then(|ni| figure.def.nodes.get(ni).map(|node| (ni, node)));
                    let is_origin = selected
                        .map(|(_, node)| node.parent.is_none())
                        .unwrap_or(false);
                    let angle_deg = selected.and_then(|(ni, node)| {
                        let parent = node.parent?;
                        let positions = core.positions(core.current, core.selected_figure);
                        let start = *positions.get(parent)?;
                        let end = *positions.get(ni)?;
                        Some((end[1] - start[1]).atan2(end[0] - start[0]).to_degrees())
                    });
                    crate::ui_inspector::InspectorData {
                        figure_name: figure.name.clone(),
                        figure_color: figure.color,
                        node_index: selected.map(|(ni, _)| ni),
                        seg_count: figure.def.nodes.len().saturating_sub(1),
                        length: selected.and_then(|(_, node)| {
                            node.parent.map(|_| node.length)
                        }),
                        thickness: selected.and_then(|(_, node)| {
                            node.parent.map(|_| node.thickness)
                        }),
                        angle_deg,
                        is_origin,
                    }
                }
                None => crate::ui_inspector::InspectorData {
                    figure_name: "No figure".to_string(),
                    figure_color: 0,
                    node_index: None,
                    seg_count: 0,
                    length: None,
                    thickness: None,
                    angle_deg: None,
                    is_origin: false,
                },
            };
            let has_segment = inspector_data.node_index.is_some() && !inspector_data.is_origin;
            let mut panel = crate::ui_inspector::inspector_panel(inspector_data);
            if has_segment {
                panel = panel
                    .child(section_label("Adjust"))
                    .child(
                        group_row()
                            .child(btn("insp-len-dn", "Len -", false, cx.listener(|view, _ev, _w, cx| {
                                view.core.adjust_length(-4.0);
                                cx.notify();
                            })))
                            .child(btn("insp-len-up", "Len +", false, cx.listener(|view, _ev, _w, cx| {
                                view.core.adjust_length(4.0);
                                cx.notify();
                            }))),
                    )
                    .child(
                        group_row()
                            .child(btn("insp-th-dn", "Thick -", false, cx.listener(|view, _ev, _w, cx| {
                                view.core.adjust_thickness(-1.0);
                                cx.notify();
                            })))
                            .child(btn("insp-th-up", "Thick +", false, cx.listener(|view, _ev, _w, cx| {
                                view.core.adjust_thickness(1.0);
                                cx.notify();
                            }))),
                    );
            }
            Some(panel)
        } else {
            None
        };

        let middle = div()
            .flex()
            .flex_row()
            .flex_1()
            .gap(px(0.0))
            .child(sidebar);
        // Canvas HUD state: mode follows the canvas's own claim
        // order — an active 3D session owns the stage (its bake
        // preview is what the canvas paints), then draw mode,
        // then camera mode, then plain posing (the input handlers
        // check draw before camera the same way).
        let hud_state = HudState {
            mode: if core.fig3.is_some() {
                "3D".to_string()
            } else if draw_on {
                "Draw".to_string()
            } else if cam_mode {
                "Camera".to_string()
            } else {
                "Pose".to_string()
            },
            tool: if draw_on {
                Some(if core.draw_tool_is_eraser() {
                    "Eraser".to_string()
                } else {
                    format!("Brush: {}", core.active_brush().name)
                })
            } else {
                None
            },
            camera: format!("Cam ({ccx:.0}, {ccy:.0}) x{czoom:.2}"),
            weather: core.project.weather.as_ref().map(|w| {
                let kind = match w.kind {
                    crate::vfx::WeatherKind::Rain => "Rain",
                    crate::vfx::WeatherKind::Snow => "Snow",
                    crate::vfx::WeatherKind::Fireflies => "Fireflies",
                };
                format!("{kind} · {:.0}", w.amount)
            }),
            loop_range: core.loop_range,
            onion: onion_on,
        };
        // The stage column: HUD strip over the framed canvas, a
        // quiet hint row under it, then the graph / audio panels.
        let framed_canvas = div()
            .flex()
            .flex_1()
            .border_1()
            .border_color(rgb(C_BORDER))
            .rounded(px(8.0))
            .overflow_hidden()
            .child(canvas);
        let hint_row = div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .items_center()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(C_MUTED))
                    .child("Drag a joint to pose · Hands and feet use IK · Drag the hips to move"),
            );
        let stage_col = div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .p(px(8.0))
            .gap(px(6.0))
            .child(hud_strip(&hud_state))
            .child(framed_canvas)
            .child(hint_row);
        let stage_col = match graph_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match audio_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match clips_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match fx_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match help_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match color_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match weather_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match export_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let stage_col = match easing_panel {
            Some(panel) => stage_col.child(panel),
            None => stage_col,
        };
        let middle = match home_panel {
            Some(panel) => middle.child(panel),
            None => middle.child(stage_col),
        };
        let middle = match inspector_panel {
            Some(panel) => middle.child(panel),
            None => middle,
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
                // Text authoring captures the keyboard while active.
                if view.text_edit.is_some() {
                    match key {
                        "enter" => {
                            let te = view.text_edit.take().unwrap();
                            view.core.status = if te.commit(&mut view.core.project) {
                                "Text set.".into()
                            } else {
                                "Text target no longer exists.".into()
                            };
                        }
                        "escape" => {
                            view.text_edit = None;
                            view.core.status = "Text edit cancelled.".into();
                        }
                        "backspace" => {
                            let te = view.text_edit.as_mut().unwrap();
                            te.backspace();
                            view.core.status = format!("Text: {}", te.display());
                        }
                        "delete" => {
                            let te = view.text_edit.as_mut().unwrap();
                            te.delete();
                            view.core.status = format!("Text: {}", te.display());
                        }
                        "left" => view.text_edit.as_mut().unwrap().move_cursor(-1),
                        "right" => view.text_edit.as_mut().unwrap().move_cursor(1),
                        "home" => view.text_edit.as_mut().unwrap().home(),
                        "end" => view.text_edit.as_mut().unwrap().end(),
                        _ => {
                            if !ctrl {
                                if let Some(kc) = &ev.keystroke.key_char {
                                    let te = view.text_edit.as_mut().unwrap();
                                    for c in kc.chars() {
                                        te.push_char(c);
                                    }
                                    view.core.status = format!("Text: {}", te.display());
                                }
                            }
                        }
                    }
                    cx.notify();
                    return;
                }
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
                // The graph panel gets first claim on presses
                // inside it (Ctrl+click deletes a key there).
                if let Some(local) = view.graph_local(ev.position, false) {
                    if ev.modifiers.control {
                        view.graph_delete_at(local);
                    } else {
                        view.graph_press(local);
                    }
                    cx.notify();
                    return;
                }
                let p = view.to_stage(ev.position);
                if view.core.draw_mode {
                    view.core.begin_draw(p);
                    cx.notify();
                } else if view.core.camera_mode {
                    view.core.begin_camera_drag(p);
                    cx.notify();
                } else if let Some(pd) = view.path_point_hit(p) {
                    // A press landing on a motion-path vertex of
                    // the selected figure starts a path drag,
                    // which takes precedence over pose dragging.
                    // The recogniser is not fed, so no pose drag
                    // or tap-select starts underneath it.
                    view.path_drag = Some(pd);
                    cx.notify();
                } else {
                    // Pose mode goes through the gesture
                    // recogniser (mouse = MOUSE_POINTER_ID).
                    view.feed_pointer(MOUSE_POINTER_ID, PointerKind::Down, p);
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(|view, ev: &MouseDownEvent, _w, cx| {
                // Right-click in the graph panel deletes a key.
                if let Some(local) = view.graph_local(ev.position, false) {
                    view.graph_delete_at(local);
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|view, ev: &MouseMoveEvent, _w, cx| {
                // An active graph tangent drag owns all moves.
                if view.graph_tan_drag.is_some() {
                    if let Some(local) = view.graph_local(ev.position, true) {
                        view.graph_tan_drag_to(local);
                        cx.notify();
                    }
                    return;
                }
                // An active graph-key drag owns all moves.
                if view.graph_drag.is_some() {
                    if let Some(local) = view.graph_local(ev.position, true) {
                        view.graph_drag_to(local);
                        cx.notify();
                    }
                    return;
                }
                // An active path-point drag owns all moves too,
                // even if a mode is toggled mid-drag (mirrors the
                // camera drag's ownership below). The delta is
                // recomputed from the press point each move, so
                // it accumulates without drift; draw_data
                // previews it, end_path_drag applies it.
                if view.path_drag.is_some() {
                    let p = view.to_stage(ev.position);
                    if let Some(pd) = &mut view.path_drag {
                        pd.delta = [p[0] - pd.start[0], p[1] - pd.start[1]];
                    }
                    cx.notify();
                    return;
                }
                if view.core.draw_mode {
                    let p = view.to_stage(ev.position);
                    view.core.draw_to(p);
                    cx.notify();
                } else if matches!(view.core.dragging, Some(crate::editor::DragKind::Camera)) {
                    // A camera drag in progress owns the move,
                    // even if camera mode was toggled mid-drag.
                    let p = view.to_stage(ev.position);
                    view.core.drag_to(p);
                    cx.notify();
                } else if !view.core.camera_mode {
                    let p = view.to_stage(ev.position);
                    if view.feed_pointer(MOUSE_POINTER_ID, PointerKind::Move, p) {
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|view, ev: &MouseUpEvent, _w, cx| {
                if view.graph_tan_drag.is_some() {
                    view.graph_tan_release();
                    cx.notify();
                    return;
                }
                if view.graph_drag.is_some() {
                    view.graph_release();
                    cx.notify();
                    return;
                }
                if view.path_drag.is_some() {
                    view.end_path_drag();
                    cx.notify();
                    return;
                }
                if view.core.draw_mode {
                    view.core.end_draw();
                    cx.notify();
                } else if matches!(view.core.dragging, Some(crate::editor::DragKind::Camera)) {
                    view.core.end_drag();
                    cx.notify();
                } else {
                    let p = view.to_stage(ev.position);
                    view.feed_pointer(MOUSE_POINTER_ID, PointerKind::Up, p);
                    if view.core.dragging.is_some() {
                        // A drag the recogniser does not own
                        // (input mode was toggled mid-drag):
                        // end it here rather than leaving it stuck.
                        view.core.end_drag();
                    }
                    cx.notify();
                }
            }))
            .child(
                top_bar()
                    .p(px(8.0))
                    .bg(rgb(bar_bg))
                    .child(brand())
                    .child(vsep())
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
                    .child(div().flex_1())
                    .child(btn("graph", "Graph", graph_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_graph = !view.show_graph;
                        if view.show_graph {
                            view.show_audio = false;
                            view.show_fx = false;
                        }
                        if !view.show_graph {
                            view.graph_drag = None;
                        }
                        cx.notify();
                    })))
                    .child(btn("audio-panel", "Audio", audio_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_audio = !view.show_audio;
                        if view.show_audio {
                            view.show_graph = false;
                            view.graph_drag = None;
                            view.show_fx = false;
                        }
                        cx.notify();
                    })))
                    .child(btn("fx-panel", "FX", fx_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_fx = !view.show_fx;
                        if view.show_fx {
                            view.show_graph = false;
                            view.graph_drag = None;
                            view.show_audio = false;
                        }
                        cx.notify();
                    })))
                    .child(btn("inspector", "Inspector", inspector_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_inspector = !view.show_inspector;
                        if view.show_inspector {
                            view.show_graph = false;
                            view.graph_drag = None;
                            view.show_audio = false;
                            view.show_fx = false;
                            view.show_clips = false;
                        }
                        cx.notify();
                    })))
                    .child(btn("clips-panel", "Clips", clips_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_clips = !view.show_clips;
                        if view.show_clips {
                            view.show_graph = false;
                            view.show_audio = false;
                            view.graph_drag = None;
                        }
                        cx.notify();
                    })))
                    .child(btn("export-panel", "Export", export_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_export = !view.show_export;
                        if view.show_export {
                            view.show_graph = false;
                            view.show_audio = false;
                            view.show_fx = false;
                            view.show_clips = false;
                            view.graph_drag = None;
                        }
                        cx.notify();
                    })))
                    .child(info_chip(frame_label.clone(), false))
                    .child(info_chip(fps_label.clone(), false))
                    .child(btn("help", "?", help_on, cx.listener(|view, _ev, _w, cx| {
                        view.show_help = !view.show_help;
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(bar_bg))
                    .child(
                        tool_group("Transport").child(
                            group_row()
                                .child(btn("go-first", "|<", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.goto(0);
                                    cx.notify();
                                })))
                                .child(btn("go-prev", "<", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.goto(view.core.current.saturating_sub(1));
                                    cx.notify();
                                })))
                                .child(btn("play", if playing { tr("pause") } else { tr("play") }, playing, cx.listener(|view, _ev, _w, cx| {
                                    view.toggle_play(cx);
                                })))
                                .child(btn("go-next", ">", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.goto(view.core.current + 1);
                                    cx.notify();
                                })))
                                .child(btn("go-last", ">|", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.goto(view.core.frame_count().saturating_sub(1));
                                    cx.notify();
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
                                .child(btn("loop-start", "Loop start", core.loop_range.is_some(), cx.listener(|view, _ev, _w, cx| {
                                    let cur = view.core.current;
                                    let (_, b) = view.core.loop_range.unwrap_or((cur, cur));
                                    view.core.set_loop_range(cur, b);
                                    cx.notify();
                                })))
                                .child(btn("loop-end", "Loop end", core.loop_range.is_some(), cx.listener(|view, _ev, _w, cx| {
                                    let cur = view.core.current;
                                    let (a, _) = view.core.loop_range.unwrap_or((cur, cur));
                                    view.core.set_loop_range(a, cur);
                                    cx.notify();
                                })))
                                .child(btn("loop-clear", "Loop clear", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.clear_loop_range();
                                    cx.notify();
                                }))))
                    )
                    .child(
                        tool_group("Figure").child(
                            group_row()
                                .child(btn("add-figure", tr("add_figure"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_figure();
                                    cx.notify();
                                }))))
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_start()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(row_bg))
                    .child(
                        tool_group("Frames").child(
                            group_row()
                                .child(btn("inbetween", tr("inbetween"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.insert_inbetween();
                                    cx.notify();
                                })))
                                .child(btn("shift-back", "< Shift", false, cx.listener(|view, _ev, _w, cx| {
                                    let cur = view.core.current;
                                    if !view.core.shift_frames(cur, 1, -1) {
                                        view.core.status = "First frame: cannot shift it earlier.".into();
                                    }
                                    cx.notify();
                                })))
                                .child(btn("shift-fwd", "Shift >", false, cx.listener(|view, _ev, _w, cx| {
                                    let cur = view.core.current;
                                    if !view.core.shift_frames(cur, 1, 1) {
                                        view.core.status = "Last frame: cannot shift it later.".into();
                                    }
                                    cx.notify();
                                })))
                                .child(btn("reverse-range", "Reverse", false, cx.listener(|view, _ev, _w, cx| {
                                    match view.core.loop_range {
                                        Some((a, b)) => view.core.reverse_range(a, b - a + 1),
                                        None => {
                                            let n = view.core.frame_count();
                                            view.core.reverse_range(0, n);
                                        }
                                    }
                                    cx.notify();
                                })))
                                .child(btn("smooth-path", "Smooth path", false, cx.listener(|view, _ev, _w, cx| {
                                    let fi = view.core.selected_figure;
                                    view.core.smooth_path_roots(fi);
                                    cx.notify();
                                })))
                                .child(btn("range-copy", "Copy range", false, cx.listener(|view, _ev, _w, cx| {
                                    // No selection falls back to the
                                    // current frame on its own.
                                    match view.range_sel {
                                        Some((a, f)) => {
                                            let (lo, hi) = if a <= f { (a, f) } else { (f, a) };
                                            view.core.copy_range(lo, hi - lo + 1);
                                        }
                                        None => {
                                            let cur = view.core.current;
                                            view.core.copy_range(cur, 1);
                                        }
                                    }
                                    cx.notify();
                                })))
                                .child(btn("range-paste", "Paste range", false, cx.listener(|view, _ev, _w, cx| {
                                    let at = view.core.current;
                                    view.core.paste_range(at);
                                    cx.notify();
                                })))
                                .child(btn("range-del", "Delete range", false, cx.listener(|view, _ev, _w, cx| {
                                    // No selection: a no-op (the kit
                                    // has no disabled state); the
                                    // selection clears after a delete
                                    // because its indices are stale.
                                    if let Some((a, f)) = view.range_sel {
                                        let (lo, hi) = if a <= f { (a, f) } else { (f, a) };
                                        if view.core.delete_range(lo, hi - lo + 1) {
                                            view.range_sel = None;
                                        }
                                    }
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Segments").child(
                            group_row()
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
                                .child(btn("color-panel", "Colour", color_on, cx.listener(|view, _ev, _w, cx| {
                                    view.show_color = !view.show_color;
                                    cx.notify();
                                })))
                                .child(btn("node-shape", tr("shape"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_shape();
                                    cx.notify();
                                })))
                                .child(btn("cycle-bend", "Bend", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_bend();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Figures").child(
                            group_row()
                                .child(btn("add-quad", "+ Quadruped", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_quadruped();
                                    cx.notify();
                                })))
                                .child(btn("add-mini", "+ Mini", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_simple();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Draw").child(
                            group_row()
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
                                .child(btn("brush-cycle", Box::leak(format!("Brush: {}", core.active_brush().name).into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_brush();
                                    cx.notify();
                                })))
                                .child(btn("draw-tool", Box::leak(format!("Tool: {}", if core.draw_tool_is_eraser() { "Eraser" } else { "Brush" }).into_boxed_str()) as &str, core.draw_tool_is_eraser(), cx.listener(|view, _ev, _w, cx| {
                                    view.core.toggle_draw_tool();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Camera").child(
                            group_row()
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
                                .child(info_chip(cam_label, false))
                        ),
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
                    .child(
                        tool_group("Playback").child(
                            group_row()
                                .child(btn("fps-dn", "FPS -", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.adjust_fps(-1.0);
                                    cx.notify();
                                })))
                                .child(btn("fps-up", "FPS +", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.adjust_fps(1.0);
                                    cx.notify();
                                })))
                                .child(btn("fps-preset", Box::leak(fps_preset_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                                    // Cycle through the preset frame rates:
                                    // jump to the next preset strictly above
                                    // the current fps, wrapping back to 6
                                    // past 60. Applied through adjust_fps
                                    // (as a delta) so playback timing and
                                    // the status line stay consistent with
                                    // the FPS +/- buttons.
                                    const PRESETS: [f32; 5] = [6.0, 12.0, 24.0, 30.0, 60.0];
                                    let cur = view.core.project.fps;
                                    let next = PRESETS
                                        .iter()
                                        .copied()
                                        .find(|p| *p > cur)
                                        .unwrap_or(PRESETS[0]);
                                    view.core.adjust_fps(next - cur);
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
                        ),
                    )
                    .child(
                        tool_group("Effects").child(
                            group_row()
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
                                .child(btn("paths", "Paths", paths_on, cx.listener(|view, _ev, _w, cx| {
                                    view.core.show_paths = !view.core.show_paths;
                                    cx.notify();
                                })))
                                .child(btn("follow", tr("follow_cam"), follow_on, cx.listener(|view, _ev, _w, cx| {
                                    view.core.toggle_camera_follow();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Library & Clips").child(
                            group_row()
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
                                .child(btn("make-clip", tr("make_clip"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.make_clip_from_figure();
                                    cx.notify();
                                })))
                                .child(btn("insert-clip", tr("insert_clip"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.insert_clip_figure();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Export").child(
                            group_row()
                                .child(btn("export-mp4", "Export MP4", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_mp4();
                                    cx.notify();
                                })))
                                .child(btn("export-sheet", tr("spritesheet"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_sheet();
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
                                .child(btn("export-piv", tr("export_piv"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_piv();
                                    cx.notify();
                                })))
                                .child(btn("export-stk", tr("export_stk"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_stk();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Projects").child(
                            group_row()
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
                                })))
                                .child(btn("open-dlg", "Open...", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.open_via_dialog();
                                    cx.notify();
                                })))
                                .child(btn("save-dlg", "Save...", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.save_via_dialog();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Import").child(
                            group_row()
                                .child(btn("import-piv", tr("import_piv"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.import_piv();
                                    cx.notify();
                                })))
                                .child(btn("import-stk", tr("import_stk"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.import_stk();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("View").child(
                            group_row()
                                .child(btn("theme", if core.light_theme { tr("dark_theme") } else { tr("light_theme") }, false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.light_theme = !view.core.light_theme;
                                    cx.notify();
                                })))
                                .child(btn("onion-range", "Onion +", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_onion_frames();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Audio & Text").child(
                            group_row()
                                .child(btn("add-audio", tr("add_audio"), false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_audio_clip();
                                    cx.notify();
                                })))
                                .child(btn("text-seg", "Text seg", false, cx.listener(|view, _ev, _w, cx| {
                                    match view.core.selected_node {
                                        Some(ni) => match crate::text_edit::TextEdit::begin(&view.core.project, view.core.selected_figure, ni) {
                                            Some(te) => {
                                                view.core.status = format!("Text: {}", te.display());
                                                view.text_edit = Some(te);
                                            }
                                            None => view.core.status = "No such segment.".into(),
                                        },
                                        None => view.core.status = "Click a joint first, then Text seg.".into(),
                                    }
                                    cx.notify();
                                })))
                                .child(btn("key-graph", "Key graph", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.key_graph_angle();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("3D").child(
                            group_row()
                                .child(btn("mode-3d", "3D mode", self.core.fig3.is_some(), cx.listener(|view, _ev, _w, cx| {
                                    if view.core.fig3.is_some() {
                                        view.core.exit_3d();
                                    } else {
                                        view.core.enter_3d();
                                    }
                                    cx.notify();
                                })))
                                .child(btn("joint-3d", "Joint>", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_select_next();
                                    cx.notify();
                                })))
                                .child(btn("rotx-3d", "RotX", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_rotate(0, 15.0);
                                    cx.notify();
                                })))
                                .child(btn("roty-3d", "RotY", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_rotate(1, 15.0);
                                    cx.notify();
                                })))
                                .child(btn("rotz-3d", "RotZ", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_rotate(2, 15.0);
                                    cx.notify();
                                })))
                                .child(btn("orbit-3d", "Orbit", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_orbit(15.0, 0.0);
                                    cx.notify();
                                })))
                                .child(btn("bake-3d", "Bake 3D", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_bake();
                                    cx.notify();
                                })))
                                .child(btn("mark-b-3d", "Mark B", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_mark_pose_b();
                                    cx.notify();
                                })))
                                .child(btn("bake-anim-3d", "Bake anim", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_bake_animation(8);
                                    cx.notify();
                                })))
                                .child(btn("save-3d", "Save 3D", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_save_to_library();
                                    cx.notify();
                                })))
                                .child(btn("load-3d", "Load 3D", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.fig3_cycle_library();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Scene").child(
                            group_row()
                                .child(btn("weather", "Weather", false, cx.listener(|view, _ev, _w, cx| {
                                    use crate::vfx::{Weather, WeatherKind};
                                    let (next, label) = match &view.core.project.weather {
                                        None => (Some(Weather { kind: WeatherKind::Rain, amount: 90.0, wind: 25.0 }), "Weather: rain."),
                                        Some(w) => match w.kind {
                                            WeatherKind::Rain => (Some(Weather { kind: WeatherKind::Snow, amount: 90.0, wind: 12.0 }), "Weather: snow."),
                                            WeatherKind::Snow => (Some(Weather { kind: WeatherKind::Fireflies, amount: 60.0, wind: 4.0 }), "Weather: fireflies."),
                                            WeatherKind::Fireflies => (None, "Weather: off."),
                                        },
                                    };
                                    view.core.project.weather = next;
                                    view.core.status = label.into();
                                    cx.notify();
                                })))
                                .child(btn("weather-panel", "Weather", weather_on, cx.listener(|view, _ev, _w, cx| {
                                    view.show_weather = !view.show_weather;
                                    cx.notify();
                                })))
                                .child(btn("cam-rot", "Cam rot +15", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.rotate_camera(15.0);
                                    cx.notify();
                                })))
                                .child(btn("clr-graph", "Clr graph", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.clear_graph();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Audio Ops").child(
                            group_row()
                                .child(btn("norm-audio", "Norm last", false, cx.listener(|view, _ev, _w, cx| {
                                    let n = view.core.project.audio.len();
                                    if n == 0 {
                                        view.core.status = "No audio clips.".into();
                                    } else {
                                        view.core.audio_normalize(n - 1);
                                    }
                                    cx.notify();
                                })))
                                .child(btn("split-audio", "Split@cur", false, cx.listener(|view, _ev, _w, cx| {
                                    let n = view.core.project.audio.len();
                                    if n == 0 {
                                        view.core.status = "No audio clips.".into();
                                    } else {
                                        let at = view.core.current;
                                        view.core.audio_split(n - 1, at);
                                    }
                                    cx.notify();
                                })))
                                .child(btn("xfade-audio", "Xfade", false, cx.listener(|view, _ev, _w, cx| {
                                    if view.core.project.audio.len() < 2 {
                                        view.core.status = "Need two audio clips to crossfade.".into();
                                    } else {
                                        view.core.audio_crossfade(0);
                                    }
                                    cx.notify();
                                })))
                                .child(btn("mixdown", "Mixdown", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.mixdown_audio();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Formats").child(
                            group_row()
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
                                .child(btn("export-v2", "Export .piv v2", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_piv2();
                                    cx.notify();
                                })))
                                .child(btn("export-v4", "Export .piv v4", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_piv4();
                                    cx.notify();
                                })))
                                .child(btn("export-nodes", "Export .nodes", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_nodes();
                                    cx.notify();
                                })))
                                .child(btn("export-stknds", "Export .stknds", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.export_stknds();
                                    cx.notify();
                                })))
                                .child(btn("import-nodes", "Import .nodes", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.import_nodes_file();
                                    cx.notify();
                                })))
                                .child(btn("import-stknds", "Import .stknds", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.import_nodes_anim_file();
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Stage & Language").child(
                            group_row()
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
                                .child(btn("lang", Box::leak(lang_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_lang();
                                    cx.notify();
                                })))
                                .child(btn("frame-easing", Box::leak(fease_label.into_boxed_str()) as &str, false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.cycle_frame_easing();
                                    cx.notify();
                                })))
                                .child(btn("easing-panel", "Easing", easing_on, cx.listener(|view, _ev, _w, cx| {
                                    view.show_easing = !view.show_easing;
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Effects 2").child(
                            group_row()
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
                                .child(btn("fxclip-meteor", "FX clip Meteor", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_fx_clip(crate::vfx::ClipKind::Meteor);
                                    cx.notify();
                                })))
                                .child(btn("fxclip-petals", "FX clip Petals", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_fx_clip(crate::vfx::ClipKind::Petals);
                                    cx.notify();
                                })))
                                .child(btn("fxclip-splash", "FX clip Splash", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_fx_clip(crate::vfx::ClipKind::Splash);
                                    cx.notify();
                                })))
                                .child(btn("fxclip-spiral", "FX clip Spiral", false, cx.listener(|view, _ev, _w, cx| {
                                    view.core.add_fx_clip(crate::vfx::ClipKind::Spiral);
                                    cx.notify();
                                })))
                        ),
                    )
                    .child(
                        tool_group("Home").child(
                            group_row()
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
                        ),
                    )
                    .child(
                        tool_group("Segment Style").child(
                            group_row()
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
                        ),
                    )
                    .child(
                        tool_group("Strokes").child(
                            group_row()
                                .child(btn("smooth-strokes", "Smooth", false, cx.listener(|view, _ev, _w, cx| {
                                    let n = crate::draw_ops::smooth_frame(&mut view.core.project, view.core.current, 1);
                                    view.core.status = format!("Smoothed {n} stroke(s) on this frame.");
                                    cx.notify();
                                })))
                        ),
                    )
            )
            .child(middle)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(rgb(bar_bg))
                    .child(dopesheet)
                    .child(ruler)
                    .child(scrubber)
                    .child(
                        top_bar()
                            .child(section_label("Timeline"))
                            .child(timeline.flex_1()),
                    )
                    .child(status_el),
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

/// Paint the graph panel: background, zero line, playhead, the
/// channel curve as connected segments and its key handles as
/// small squares, with the axis box on top. All data<->pixel
/// mapping goes through `GraphView` (auto_fit + curve_points).
/// Pixel length of a graph tangent handle's arm.
const GRAPH_TAN_HANDLE_PX: f32 = 34.0;

/// Unit pixel direction of a Hermite tangent slope under the
/// panel view's mapping: one frame maps to `width / frame span`
/// px horizontally, one value unit to `height / value span` px
/// vertically (pixel y grows down, so the value component is
/// negated). A zero slope points straight right; a degenerate
/// view falls back to that instead of NaN.
fn graph_tan_dir(view: &crate::graph_panel::GraphView, tan: f32) -> V2 {
    let sx = if view.frame_max > view.frame_min {
        view.width / (view.frame_max - view.frame_min)
    } else {
        0.0
    };
    let sy = if view.val_max > view.val_min {
        view.height / (view.val_max - view.val_min)
    } else {
        0.0
    };
    let v = [sx, -tan * sy];
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len > 1e-6 {
        [v[0] / len, v[1] / len]
    } else {
        [1.0, 0.0]
    }
}

/// The (incoming, outgoing) tangent-handle pixel positions for
/// key `k`: one arm length from the key point — backwards along
/// the incoming tangent line, forwards along the outgoing one.
/// `graph_press` hit-tests exactly these positions.
fn graph_tan_handles(view: &crate::graph_panel::GraphView, k: &crate::graph::Key) -> (V2, V2) {
    let c = view.to_px(k.frame, k.value);
    let di = graph_tan_dir(view, k.in_tan);
    let dout = graph_tan_dir(view, k.out_tan);
    (
        [
            c[0] - di[0] * GRAPH_TAN_HANDLE_PX,
            c[1] - di[1] * GRAPH_TAN_HANDLE_PX,
        ],
        [
            c[0] + dout[0] * GRAPH_TAN_HANDLE_PX,
            c[1] + dout[1] * GRAPH_TAN_HANDLE_PX,
        ],
    )
}

fn paint_graph(window: &mut Window, bounds: Bounds<Pixels>, data: &GraphDrawData) {
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let view = crate::graph_panel::GraphView::auto_fit(
        std::slice::from_ref(&data.fit),
        data.frames,
        w,
        h,
    );
    let at = |p: V2| point(origin.x + px(p[0]), origin.y + px(p[1]));
    // Background.
    fill_rect(window, origin, w, h, rgb(data.bg));
    // Zero line, when zero is inside the value range.
    if view.val_min < 0.0 && view.val_max > 0.0 {
        let y = view.to_px(view.frame_min, 0.0)[1];
        stroke_line(window, at([0.0, y]), at([w, y]), rgba(0x9AA3B2 << 8 | 0x55), 1.0);
    }
    // Playhead at the editor's current frame.
    let play_x = view.to_px(data.current, view.val_min)[0];
    if (0.0..=w).contains(&play_x) {
        stroke_line(
            window,
            at([play_x, 0.0]),
            at([play_x, h]),
            rgba(0xFF5D5D << 8 | 0x99),
            1.0,
        );
    }
    // Curve + key handles.
    if let Some(ch) = &data.channel {
        if !ch.keys.is_empty() {
            let pts = view.curve_points(ch, 120);
            let mut pb = PathBuilder::stroke(px(2.0));
            for (i, p) in pts.iter().enumerate() {
                if i == 0 {
                    pb.move_to(at(*p));
                } else {
                    pb.line_to(at(*p));
                }
            }
            if let Ok(path) = pb.build() {
                window.paint_path(path, rgb(0x35C4D8));
            }
            for (i, hp) in view.key_handles(ch).iter().enumerate() {
                let dragged = data
                    .drag_frame
                    .map(|f| (ch.keys[i].frame - f).abs() < 0.01)
                    .unwrap_or(false);
                let c = at(*hp);
                let half = 4.0;
                fill_rect(
                    window,
                    point(c.x - px(half), c.y - px(half)),
                    half * 2.0,
                    half * 2.0,
                    rgb(if dragged { 0xFFC65D } else { 0xE8E8EC }),
                );
            }
            // Tangent arms + round handles on the selected key.
            if let Some(sel) = data.sel_frame {
                if let Some(key) = ch.keys.iter().find(|k| (k.frame - sel).abs() < 0.01) {
                    let (hin, hout) = graph_tan_handles(&view, key);
                    let kc = view.to_px(key.frame, key.value);
                    for hp in [hin, hout] {
                        stroke_line(window, at(kc), at(hp), rgba(0xFFC65D << 8 | 0x88), 1.0);
                        let mut pb = PathBuilder::fill();
                        trace_circle(&mut pb, at(hp), 3.5);
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, rgb(0xFFC65D));
                        }
                    }
                }
            }
        }
    }
    // Axis box on top.
    let mut pb = PathBuilder::stroke(px(1.0));
    pb.move_to(origin);
    pb.line_to(point(origin.x + px(w), origin.y));
    pb.line_to(point(origin.x + px(w), origin.y + px(h)));
    pb.line_to(point(origin.x, origin.y + px(h)));
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, rgb(0x3A4150));
    }
}

/// Cache-key stamp for an audio file: its mtime in nanoseconds
/// since the epoch, or 0 when the metadata can't be read (a
/// missing file then keys as (path, 0) and probes as unreadable).
fn audio_file_stamp(path: &str) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() * 1_000_000_000 + d.subsec_nanos() as u64)
        .unwrap_or(0)
}

/// Probe one WAV for the audio panel: 96 peak buckets + duration.
/// `None` when the file is missing or not a readable WAV.
fn probe_audio_peaks(path: &str) -> Option<AudioPeaksEntry> {
    let data = crate::audio_edit::read_wav(path).ok()?;
    Some(AudioPeaksEntry {
        peaks: crate::audio_edit::peaks(&data, 96),
        duration_secs: data.duration_secs(),
    })
}

/// Paint one clip's waveform strip: background, a centre line,
/// one vertical bar per peak bucket (amplitude mirrored around
/// the centre), and the playhead line when the editor's current
/// frame falls inside the clip's span.
fn paint_audio_peaks(window: &mut Window, bounds: Bounds<Pixels>, data: &AudioStripData) {
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    fill_rect(window, origin, w, h, rgb(data.bg));
    let mid = h / 2.0;
    stroke_line(
        window,
        point(origin.x, origin.y + px(mid)),
        point(origin.x + px(w), origin.y + px(mid)),
        rgba(0x9AA3B2 << 8 | 0x44),
        1.0,
    );
    let n = data.peaks.len();
    if n > 0 {
        let step = w / n as f32;
        let bar_w = step.max(1.0);
        let mut pb = PathBuilder::fill();
        for (i, &pk) in data.peaks.iter().enumerate() {
            let amp = pk.clamp(0.0, 1.0) * (mid - 1.0).max(0.0);
            let x = i as f32 * step;
            let tl = point(origin.x + px(x), origin.y + px(mid - amp));
            pb.move_to(tl);
            pb.line_to(point(origin.x + px(x + bar_w), origin.y + px(mid - amp)));
            pb.line_to(point(origin.x + px(x + bar_w), origin.y + px(mid + amp)));
            pb.line_to(point(origin.x + px(x), origin.y + px(mid + amp)));
            pb.close();
        }
        if let Ok(path) = pb.build() {
            window.paint_path(path, rgb(0x35C4D8));
        }
    }
    if let Some(frac) = data.playhead {
        let x = frac.clamp(0.0, 1.0) * w;
        stroke_line(
            window,
            point(origin.x + px(x), origin.y),
            point(origin.x + px(x), origin.y + px(h)),
            rgba(0xFF5D5D << 8 | 0xCC),
            1.0,
        );
    }
}

/// One straight stroked segment between two window points.
fn stroke_line(window: &mut Window, a: Point<Pixels>, b: Point<Pixels>, color: Rgba, width: f32) {
    let mut pb = PathBuilder::stroke(px(width));
    pb.move_to(a);
    pb.line_to(b);
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

/// A filled rectangle from `origin` spanning `w` x `h` pixels.
fn fill_rect(window: &mut Window, origin: Point<Pixels>, w: f32, h: f32, color: Rgba) {
    let mut pb = PathBuilder::fill();
    pb.move_to(origin);
    pb.line_to(point(origin.x + px(w), origin.y));
    pb.line_to(point(origin.x + px(w), origin.y + px(h)));
    pb.line_to(point(origin.x, origin.y + px(h)));
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
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

