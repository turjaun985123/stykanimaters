//! Camera panel + stage-overlay cluster for the Rune editor
//! shell (Phase D, section 6; Phase E, section 32 added the
//! camera-key list and the rotation drag dial). Bin-crate
//! code — declared under `rune_sec` by the integrator, not
//! part of the engine library.
//!
//! Every control calls exactly one real `EditorCore` camera op
//! (verified in `editor.rs`), or composes public ops/parts the
//! way the engine itself does — each composition is documented
//! where it happens:
//!
//! * Fit → `frame_all_figures` (editor.rs:582)
//! * Centre → `centre_on_selection` (editor.rs:629)
//! * 1:1, zoom steppers → `camera_zoom_by` (editor.rs:534)
//! * Rotation −15°/+15° → `rotate_camera` (editor.rs:1098)
//! * Rotation dial → [`dial_gesture`]: `rotate_camera`'s own
//!   steps composed for a drag (one `snapshot` per gesture,
//!   then field-level key writes) — see its docs for the undo
//!   contract.
//! * Set key → `rotate_camera(0.0)`: the engine's own
//!   `set_camera_key_here` upsert is private (editor.rs:555),
//!   and the other public upsert path (`camera_zoom_by` via
//!   `model::set_camera_key`) zeroes a fresh key's angle;
//!   `rotate_camera(0.0)` is the public op that upserts a key
//!   at the current frame carrying the pan/zoom/angle already
//!   in effect — exactly "set key here".
//! * Clear key → `camera_clear_key` (editor.rs:544)
//! * Key-list row → `goto` (editor.rs:681); its Clear button →
//!   `goto(frame)` then `camera_clear_key()`, because the op
//!   removes the key at the *current* frame only — see
//!   [`clear_key_at`].
//! * Follow → `toggle_camera_follow` (editor.rs:1699); its
//!   active state is `project.camera_follow ==
//!   Some(selected_figure)` (model.rs:699), the same condition
//!   the op itself toggles on.
//!
//! Engine facts this panel lives with (verified, not worked
//! around silently): `camera_clear_key` does NOT snapshot, so
//! clearing a key is not undoable; and `camera_zoom_by`'s
//! `set_camera_key` gives a *fresh* key angle 0 rather than
//! the angle in effect (key rows show the stored angle, so a
//! key created that way honestly reads 0°).
//!
//! Readouts come from `EditorCore::camera_here` (editor.rs:530)
//! and `Project::camera_angle_at` (model.rs:904) — the camera
//! in effect at the current frame, interpolated keys included.
//!
//! Two entry points: [`panel`] is the full inspector-style
//! panel (labelled rows, rotation steppers + dial, key
//! controls + key list, follow toggle); [`overlay`] is the
//! compact cluster the integrator floats over the stage
//! (Fit / Centre / 1:1 / zoom steppers + % readout). Element
//! keys reuse the GPUI-era ids where they existed (`view-fit`,
//! `view-centre`, `view-actual`).

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::State;

/// Zoom multiplier applied by one stepper press.
pub const ZOOM_STEP: f32 = 1.25;

/// Camera rotation applied by one rotation press, in degrees.
pub const ROTATE_STEP: f32 = 15.0;

/// Logical width of the rotation dial's shape (its `view_box`
/// width): one logical unit per degree across −180°..=180°.
pub const DIAL_W: f32 = 360.0;

/// Logical height of the rotation dial's shape.
pub const DIAL_H: f32 = 26.0;

/// The camera zoom in effect at the current frame, as a whole
/// percentage (100 = 1:1).
pub fn zoom_pct(core: &EditorCore) -> i64 {
    (core.camera_here().2 * 100.0).round() as i64
}

/// The zoom readout text, e.g. `"100%"`.
pub fn zoom_label(core: &EditorCore) -> String {
    format!("{}%", zoom_pct(core))
}

/// The camera angle in effect at the current frame, in
/// degrees (interpolated between keys).
pub fn angle_deg(core: &EditorCore) -> f32 {
    core.project.camera_angle_at(core.current as f32)
}

/// The angle readout text, e.g. `"0°"` / `"-15°"`.
pub fn angle_label(core: &EditorCore) -> String {
    format!("{:.0}°", angle_deg(core))
}

/// Whether the camera currently follows the selected figure —
/// the active state of the Follow toggle, matching the
/// condition `toggle_camera_follow` itself checks.
pub fn follow_active(core: &EditorCore) -> bool {
    core.project.camera_follow == Some(core.selected_figure)
}

/// The `camera_zoom_by` factor that takes `zoom` to exactly
/// 1.0 (the 1:1 / "actual size" view), or `None` when no op
/// call is needed: already at 1:1 (within a tenth of a
/// percent) or a non-positive zoom, which `camera_here` never
/// yields for a real project. Skipping the call matters —
/// `camera_zoom_by` snapshots, so a no-op 1:1 press would
/// otherwise push a dead entry onto the undo stack.
pub fn actual_factor(zoom: f32) -> Option<f32> {
    if zoom <= 0.0 || (zoom - 1.0).abs() <= 0.001 {
        None
    } else {
        Some(1.0 / zoom)
    }
}

/// 1:1 button handler: zoom to exactly 1.0 via
/// `camera_zoom_by`, unless already there.
fn zoom_actual(s: &mut State) {
    if let Some(f) = actual_factor(s.core.camera_here().2) {
        s.core.camera_zoom_by(f);
    }
}

/// "Set key here" handler: pin the camera in effect at the
/// current frame (pan, zoom and angle) as a key on this frame
/// — see the module docs for why this is `rotate_camera(0.0)`.
fn set_key_here(s: &mut State) {
    s.core.rotate_camera(0.0);
}

// ----------------------------------------------------------------
// Camera-key list
// ----------------------------------------------------------------

/// One row of the camera-key list: the *stored* key values at
/// `frame` (not the interpolated camera in effect — a row
/// describes the key itself).
#[derive(Clone, Debug, PartialEq)]
pub struct CamKeyRow {
    /// Key frame, 0-based as stored in `Project::camera`.
    pub frame: usize,
    /// The key's zoom as a whole percentage (100 = 1:1).
    pub zoom_pct: i64,
    /// The key's stored rotation, in degrees.
    pub angle: f32,
    /// Whether this key sits on the current frame (rendered
    /// highlighted; its row is the one the panel's own Clear
    /// key button would remove).
    pub is_current: bool,
}

impl CamKeyRow {
    /// The row's goto-button label, e.g. `"F3 · 150% · 15°"`
    /// (frame shown 1-based, as everywhere in the shell).
    pub fn label(&self) -> String {
        format!("F{} · {}% · {:.0}°", self.frame + 1, self.zoom_pct, self.angle)
    }
}

/// All camera keys as list rows, sorted by frame. The stored
/// `Project::camera` vec is kept sorted by the model's own
/// upserts, but a hand-edited or imported project need not be
/// — the model's evaluators sort a copy per call, and so does
/// this list.
pub fn key_rows(core: &EditorCore) -> Vec<CamKeyRow> {
    let mut rows: Vec<CamKeyRow> = core
        .project
        .camera
        .iter()
        .map(|k| CamKeyRow {
            frame: k.frame,
            zoom_pct: (k.zoom * 100.0).round() as i64,
            angle: k.angle,
            is_current: k.frame == core.current,
        })
        .collect();
    rows.sort_by_key(|r| r.frame);
    rows
}

/// Clear the camera key at `frame` (the key list's per-row
/// Clear button), as a composition of two public ops:
/// `camera_clear_key` removes the key at the *current* frame
/// only (`remove_camera_key(self.current)`, editor.rs:544),
/// so this goes to the key's frame first (`goto`,
/// editor.rs:681) and then clears. Two honest consequences:
/// the playhead ends on the cleared key's frame, and — since
/// `camera_clear_key` never snapshots — the clear is not
/// undoable (pre-existing engine behaviour, unchanged here).
/// If `frame` lies past the end of the timeline (a stale key
/// whose frames were deleted), `goto` no-ops and the clear
/// lands on the current frame instead, where the op reports
/// "No camera key on this frame." unless one exists there.
pub fn clear_key_at(core: &mut EditorCore, frame: usize) {
    core.goto(frame);
    core.camera_clear_key();
}

// ----------------------------------------------------------------
// Rotation dial
// ----------------------------------------------------------------

/// Map a dial-local x position to an absolute camera angle:
/// the track spans −180° at its left edge to +180° at its
/// right edge, linearly. Out-of-track positions clamp to the
/// ends; a degenerate (zero/negative) width maps to 0°, and
/// the gesture handler ignores such events entirely.
pub fn angle_at(x: f32, w: f32) -> f32 {
    if w <= 0.0 {
        return 0.0;
    }
    -180.0 + (x / w).clamp(0.0, 1.0) * 360.0
}

/// The dial marker's logical x for `angle` — the exact
/// inverse of [`angle_at`] in the dial's 360-wide view box.
pub fn marker_x(angle: f32) -> f32 {
    angle.clamp(-180.0, 180.0) + 180.0
}

/// Write `target` as the camera angle at the current frame,
/// field-level: the body of `rotate_camera` (editor.rs:1098)
/// minus its snapshot — upsert a key at the current frame
/// carrying the pan/zoom in effect, then set that key's
/// angle. The status line mirrors the op's own wording. The
/// dial is the only caller; it manages the gesture's single
/// snapshot itself (see [`dial_gesture`]).
fn write_angle_here(core: &mut EditorCore, target: f32) {
    let frame = core.current;
    let (x, y, zoom) = core.project.camera_at(frame as f32);
    core.project.set_camera_key(frame, x, y, zoom);
    if let Some(k) = core.project.camera.iter_mut().find(|k| k.frame == frame) {
        k.angle = target;
    }
    core.status = format!("Camera rotation {target:.0}° at frame {}.", frame + 1);
}

/// One phase of a rotation-dial drag, against the bare core
/// (the `State` handler below only unpacks the `Drag`).
///
/// Why not `rotate_camera` per Move event: the op snapshots
/// *unconditionally* (editor.rs:1099), so a one-second drag
/// would push dozens of undo steps (the stack caps at 60) for
/// what is semantically one edit. Instead the gesture
/// composes the op's own steps:
///
/// * `Down` arms the gesture: `data` (the Drag's per-gesture
///   scratch) is set to NaN — "no snapshot taken yet".
/// * The first `Down`/`Move` whose target angle actually
///   differs from the angle in effect (0.01° dead zone, so a
///   click on the current angle neither snapshots nor pins a
///   key) takes the gesture's ONE snapshot via the public
///   `EditorCore::snapshot` (editor.rs:202) and flips `data`
///   to 1.0; every apply then writes the key field-level via
///   [`write_angle_here`].
/// * `Up` with `cancelled` set (Escape, focus loss — Rune's
///   "put back whatever the drag moved") undoes that single
///   snapshot if one was taken, restoring the pre-gesture
///   state exactly; the dragged state lands on the redo
///   stack, as with any undo.
///
/// Net undo behaviour: a dial drag is exactly one undo step,
/// the same as one `rotate_camera` press.
pub fn dial_gesture(
    core: &mut EditorCore,
    phase: Phase,
    x: f32,
    w: f32,
    data: &mut f32,
    cancelled: bool,
) {
    match phase {
        Phase::Down => {
            *data = f32::NAN;
            dial_apply(core, x, w, data);
        }
        Phase::Move => dial_apply(core, x, w, data),
        Phase::Up => {
            if cancelled && *data == 1.0 {
                core.undo();
            }
            *data = f32::NAN;
        }
    }
}

/// Shared Down/Move path of [`dial_gesture`]: map the pointer
/// to an absolute target angle and, when it is a real change,
/// snapshot once per gesture and write the key.
fn dial_apply(core: &mut EditorCore, x: f32, w: f32, data: &mut f32) {
    if w <= 0.0 {
        return;
    }
    let target = angle_at(x, w);
    if (target - angle_deg(core)).abs() < 0.01 {
        return;
    }
    if data.is_nan() {
        core.snapshot();
        *data = 1.0;
    }
    write_angle_here(core, target);
}

/// The dial element's drag handler: unpack the Rune `Drag`
/// (element-local point, element width, per-gesture scratch,
/// cancellation) into [`dial_gesture`].
fn dial_drag(s: &mut State, d: &mut Drag) {
    let cancelled = d.phase == Phase::Up && d.cancelled();
    dial_gesture(&mut s.core, d.phase, d.x, d.w, &mut d.data, cancelled);
}

/// The dial's shape in its 360×26 view box: the track line,
/// ticks at −180/−90/0/90/180 (the centre tick taller) and an
/// accent diamond marker at the angle in effect. Positions
/// are view-box units (so the marker lands exactly where
/// [`angle_at`] puts the pointer); stroke widths stay in
/// element points under a view box, which is what a control
/// wants — the track and marker keep their weight at any
/// panel width.
pub fn dial_shape(core: &EditorCore, t: &Theme) -> Shape {
    let accent = tok::accent(0, true);
    let mid = DIAL_H / 2.0;
    let mut shape = Shape::new()
        .view_box(DIAL_W, DIAL_H)
        .line([0.0, mid], [DIAL_W, mid], 2.0, LineCap::Round, t.control_track);
    for a in [-180.0f32, -90.0, 0.0, 90.0, 180.0] {
        let half = if a == 0.0 { 6.0 } else { 4.0 };
        let x = marker_x(a);
        shape = shape.line(
            [x, mid - half],
            [x, mid + half],
            1.5,
            LineCap::Round,
            t.text_secondary,
        );
    }
    let mx = marker_x(angle_deg(core));
    shape.fill(
        vec![
            [mx, 3.0],
            [mx + 6.0, mid],
            [mx, DIAL_H - 3.0],
            [mx - 6.0, mid],
        ],
        accent,
    )
}

/// The rotation dial element (`cam-dial`): a wide track the
/// user drags horizontally to set the absolute camera angle
/// at the current frame. Sits under the ±15° stepper row,
/// which stays for precise nudges.
fn angle_dial(core: &EditorCore, t: &Theme) -> El {
    canvas(dial_shape(core, t))
        .key("cam-dial")
        .w_full()
        .h(DIAL_H)
        .radius(6.0)
        .fill(t.card)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| dial_drag(s, d))
}

// ----------------------------------------------------------------
// Views
// ----------------------------------------------------------------

/// The zoom readout chip (`cam-zoom-label`): the current zoom
/// percentage as centred label text.
fn zoom_readout(core: &EditorCore, t: &Theme) -> El {
    el()
        .key("cam-zoom-label")
        .px(6.0)
        .center()
        .child(text(zoom_label(core)).font(type_::LABEL).color(t.text))
}

/// The angle readout chip (`cam-angle-label`).
fn angle_readout(core: &EditorCore, t: &Theme) -> El {
    el()
        .key("cam-angle-label")
        .px(6.0)
        .center()
        .child(text(angle_label(core)).font(type_::LABEL).color(t.text))
}

/// The camera-key list section: a "Keys" header, then one row
/// per key — a goto button labelled with the key's frame,
/// zoom and angle (accent-filled when the key is on the
/// current frame) and a per-row Clear button (the goto-then-
/// clear composition in [`clear_key_at`]). With no keys, an
/// explanatory empty line instead of rows.
fn key_list(core: &EditorCore, t: &Theme) -> El {
    let rows = key_rows(core);
    let mut list = col().key("cam-key-list").gap(4.0);
    list = list.child(
        text("Keys")
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    if rows.is_empty() {
        list = list.child(
            text("No camera keys yet.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for r in rows {
        let frame = r.frame;
        list = list.child(
            row()
                .key(format!("cam-key-row-{frame}"))
                .gap(6.0)
                .center_y()
                .child(btn(
                    &format!("cam-key-{frame}"),
                    &r.label(),
                    r.is_current,
                    t,
                    move |s: &mut State| s.core.goto(frame),
                ))
                .child(btn(
                    &format!("cam-key-clear-{frame}"),
                    "Clear",
                    false,
                    t,
                    move |s: &mut State| clear_key_at(&mut s.core, frame),
                )),
        );
    }
    list
}

/// The full camera panel: a "Camera" header, the view row
/// (Fit / Centre / 1:1), the zoom stepper row, the rotation
/// stepper row, the rotation dial, the key row (Set key /
/// Clear key), the camera-key list and the follow toggle.
pub fn panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    col()
        .key("camera-panel")
        .gap(6.0)
        .child(
            text("Camera")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("view-fit", "Fit", false, t, |s: &mut State| {
                    s.core.frame_all_figures()
                }))
                .child(btn("view-centre", "Centre", false, t, |s: &mut State| {
                    s.core.centre_on_selection()
                }))
                .child(btn("view-actual", "1:1", false, t, zoom_actual)),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-zoom-out", "-", false, t, |s: &mut State| {
                    s.core.camera_zoom_by(1.0 / ZOOM_STEP)
                }))
                .child(zoom_readout(core, t))
                .child(btn("cam-zoom-in", "+", false, t, |s: &mut State| {
                    s.core.camera_zoom_by(ZOOM_STEP)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-rot-left", "-15°", false, t, |s: &mut State| {
                    s.core.rotate_camera(-ROTATE_STEP)
                }))
                .child(angle_readout(core, t))
                .child(btn("cam-rot-right", "+15°", false, t, |s: &mut State| {
                    s.core.rotate_camera(ROTATE_STEP)
                })),
        )
        .child(angle_dial(core, t))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-set-key", "Set key", false, t, set_key_here))
                .child(btn("cam-clear-key", "Clear key", false, t, |s: &mut State| {
                    s.core.camera_clear_key()
                })),
        )
        .child(key_list(core, t))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "cam-follow",
                    "Follow",
                    follow_active(core),
                    t,
                    |s: &mut State| s.core.toggle_camera_follow(),
                )),
        )
}

/// The compact stage-overlay cluster: Fit / Centre / 1:1 and
/// the zoom steppers around the % readout, on one row. The
/// integrator positions it over the stage (the audit's
/// `.absolute`/`.pin` out-of-flow placement); this builder
/// only supplies the cluster itself. Takes just the core —
/// readouts and active states need nothing else — and uses
/// the dark theme, as the shell's `view` does
/// (`tok::theme(true)` in `main.rs`).
pub fn overlay(core: &EditorCore) -> El {
    let t = tok::theme(true);
    row()
        .key("camera-overlay")
        .gap(4.0)
        .center_y()
        .child(btn("view-fit", "Fit", false, &t, |s: &mut State| {
            s.core.frame_all_figures()
        }))
        .child(btn("view-centre", "Centre", false, &t, |s: &mut State| {
            s.core.centre_on_selection()
        }))
        .child(btn("view-actual", "1:1", false, &t, zoom_actual))
        .child(btn("cam-zoom-out", "-", false, &t, |s: &mut State| {
            s.core.camera_zoom_by(1.0 / ZOOM_STEP)
        }))
        .child(zoom_readout(core, &t))
        .child(btn("cam-zoom-in", "+", false, &t, |s: &mut State| {
            s.core.camera_zoom_by(ZOOM_STEP)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_label_is_100_percent_for_a_fresh_core() {
        let core = EditorCore::new();
        assert_eq!(zoom_pct(&core), 100);
        assert_eq!(zoom_label(&core), "100%");
    }

    #[test]
    fn zoom_label_tracks_camera_zoom_by() {
        let mut core = EditorCore::new();
        core.camera_zoom_by(2.0);
        assert_eq!(zoom_label(&core), "200%");
        core.camera_zoom_by(0.25);
        assert_eq!(zoom_label(&core), "50%");
    }

    #[test]
    fn actual_factor_inverts_the_current_zoom() {
        assert_eq!(actual_factor(2.0), Some(0.5));
        assert_eq!(actual_factor(0.5), Some(2.0));
        assert_eq!(actual_factor(4.0), Some(0.25));
    }

    #[test]
    fn actual_factor_skips_the_call_when_already_actual() {
        assert_eq!(actual_factor(1.0), None);
        assert_eq!(actual_factor(1.0005), None);
        // Non-positive zooms never come from `camera_here`;
        // treat them as "nothing to do" rather than dividing.
        assert_eq!(actual_factor(0.0), None);
        assert_eq!(actual_factor(-2.0), None);
    }

    #[test]
    fn angle_label_is_zero_for_a_fresh_core() {
        let core = EditorCore::new();
        assert_eq!(angle_label(&core), "0°");
    }

    #[test]
    fn angle_label_tracks_rotate_camera() {
        let mut core = EditorCore::new();
        core.rotate_camera(ROTATE_STEP);
        assert_eq!(angle_label(&core), "15°");
        core.rotate_camera(-2.0 * ROTATE_STEP);
        assert_eq!(angle_label(&core), "-15°");
    }

    #[test]
    fn follow_active_matches_the_ops_own_condition() {
        let mut core = EditorCore::new();
        assert!(!follow_active(&core));
        core.toggle_camera_follow();
        assert!(follow_active(&core));
        // Following a different figure is not "follow active"
        // for the selected one.
        core.project.camera_follow = Some(core.selected_figure + 1);
        assert!(!follow_active(&core));
        core.toggle_camera_follow();
        assert!(follow_active(&core));
    }

    #[test]
    fn steps_are_sane() {
        assert!(ZOOM_STEP > 1.0);
        assert_eq!(ROTATE_STEP, 15.0);
    }

    // ---- Key list ----

    #[test]
    fn key_rows_empty_for_a_fresh_core() {
        let core = EditorCore::new();
        assert!(key_rows(&core).is_empty());
    }

    #[test]
    fn key_rows_report_stored_values_sorted_with_current_marked() {
        let mut core = EditorCore::new();
        core.rotate_camera(15.0); // key at frame 0: zoom 100%, angle 15
        core.goto(2);
        core.rotate_camera(30.0); // angle in effect was 15 → key at 2: 45
        let rows = key_rows(&core);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            CamKeyRow { frame: 0, zoom_pct: 100, angle: 15.0, is_current: false }
        );
        assert_eq!(
            rows[1],
            CamKeyRow { frame: 2, zoom_pct: 100, angle: 45.0, is_current: true }
        );
        assert_eq!(rows[1].label(), "F3 · 100% · 45°");
        // The current marker follows the playhead.
        core.goto(0);
        let rows = key_rows(&core);
        assert!(rows[0].is_current);
        assert!(!rows[1].is_current);
    }

    #[test]
    fn clear_key_at_goes_to_the_frame_then_clears() {
        let mut core = EditorCore::new();
        core.rotate_camera(15.0); // key at frame 0
        core.goto(2);
        core.rotate_camera(30.0); // key at frame 2
        core.goto(4);
        clear_key_at(&mut core, 0);
        // The composition's goto is observable: the playhead
        // ends on the cleared key's frame.
        assert_eq!(core.current, 0);
        assert_eq!(core.status, "Camera key removed.");
        let rows = key_rows(&core);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].frame, 2);
    }

    // ---- Rotation dial ----

    #[test]
    fn angle_at_maps_the_track_to_minus_180_through_180() {
        assert_eq!(angle_at(0.0, 200.0), -180.0);
        assert_eq!(angle_at(100.0, 200.0), 0.0);
        assert_eq!(angle_at(150.0, 200.0), 90.0);
        assert_eq!(angle_at(200.0, 200.0), 180.0);
        // Past either edge clamps to the end angles.
        assert_eq!(angle_at(-40.0, 200.0), -180.0);
        assert_eq!(angle_at(999.0, 200.0), 180.0);
        // Degenerate widths map to 0 (the gesture ignores
        // them before ever applying).
        assert_eq!(angle_at(10.0, 0.0), 0.0);
        assert_eq!(angle_at(10.0, -5.0), 0.0);
    }

    #[test]
    fn marker_x_inverts_angle_at() {
        assert_eq!(marker_x(-180.0), 0.0);
        assert_eq!(marker_x(0.0), 180.0);
        assert_eq!(marker_x(180.0), 360.0);
        for a in [-135.0f32, -45.0, 30.0, 179.0] {
            assert_eq!(angle_at(marker_x(a), DIAL_W), a);
        }
    }

    #[test]
    fn dial_drag_is_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        let mut data = 0.0f32;
        let depth0 = core.undo_depth();
        // Press at the middle: target 0° == angle in effect,
        // so no snapshot and no key is pinned.
        dial_gesture(&mut core, Phase::Down, 100.0, 200.0, &mut data, false);
        assert_eq!(core.undo_depth(), depth0);
        assert!(core.project.camera.is_empty());
        // Two moves across the track: the first snapshots,
        // the second must not.
        dial_gesture(&mut core, Phase::Move, 150.0, 200.0, &mut data, false);
        assert_eq!(angle_deg(&core), 90.0);
        dial_gesture(&mut core, Phase::Move, 200.0, 200.0, &mut data, false);
        assert_eq!(angle_deg(&core), 180.0);
        dial_gesture(&mut core, Phase::Up, 200.0, 200.0, &mut data, false);
        assert_eq!(core.undo_depth(), depth0 + 1);
        // And that one step restores the pre-drag state.
        core.undo();
        assert_eq!(angle_deg(&core), 0.0);
        assert!(core.project.camera.is_empty());
    }

    #[test]
    fn dial_click_on_a_new_angle_pins_one_key_with_one_snapshot() {
        let mut core = EditorCore::new();
        let mut data = 0.0f32;
        let depth0 = core.undo_depth();
        // A plain click (Down + Up, no Move) jumps the angle,
        // slider-style.
        dial_gesture(&mut core, Phase::Down, 50.0, 200.0, &mut data, false);
        dial_gesture(&mut core, Phase::Up, 50.0, 200.0, &mut data, false);
        assert_eq!(angle_deg(&core), -90.0);
        assert_eq!(key_rows(&core).len(), 1);
        assert_eq!(core.undo_depth(), depth0 + 1);
    }

    #[test]
    fn dial_cancelled_gesture_puts_the_angle_back() {
        let mut core = EditorCore::new();
        let mut data = 0.0f32;
        let depth0 = core.undo_depth();
        dial_gesture(&mut core, Phase::Down, 100.0, 200.0, &mut data, false);
        dial_gesture(&mut core, Phase::Move, 150.0, 200.0, &mut data, false);
        assert_eq!(angle_deg(&core), 90.0);
        // Escape mid-drag: Rune reports the Up as cancelled,
        // and the gesture's one snapshot is undone.
        dial_gesture(&mut core, Phase::Up, 150.0, 200.0, &mut data, true);
        assert_eq!(angle_deg(&core), 0.0);
        assert!(core.project.camera.is_empty());
        assert_eq!(core.undo_depth(), depth0);
    }

    #[test]
    fn panel_and_overlay_build_with_keys_and_a_dial() {
        // Smoke: the extended panel (dial + key list) builds
        // for a keyed project without panicking.
        let t = tok::theme(true);
        let mut core = EditorCore::new();
        core.rotate_camera(20.0);
        let s = State {
            core,
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
        let _ = panel(&s, &t);
        let _ = overlay(&s.core);
    }
}
