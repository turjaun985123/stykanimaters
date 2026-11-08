//! Particle events panel for the Rune editor shell (Phase G,
//! section 8). Declared from `main.rs` only — this is
//! bin-crate code, not part of the engine library.
//!
//! ## Integration contract
//!
//! This section needs NO new `State` field: `main.rs`
//! already carries
//!
//! ```ignore
//! pub particle_selected: Option<usize>,
//! ```
//!
//! (index into `Project::particles` — the same index the
//! stage FX gizmo geometry reads, so selecting a row here
//! is what makes the stage draw that event's gizmo). The
//! integrator declares this module in `rune_sec/mod.rs`,
//! adds the Section gating entry, and mounts
//! [`particles_panel`].
//!
//! ## Ops (all real; no invented engine API)
//!
//! Every mutation goes through the `EditorCore` particle
//! ops in `src/particle_ops.rs` (Phase G), which own the
//! undo snapshot:
//!
//! * add — `EditorCore::particle_add_event(frame, x, y)`
//! * delete — `EditorCore::particle_delete_event(index)`
//! * frame stepper — `EditorCore::particle_set_frame(
//!   index, frame)`
//! * kind stepper — `EditorCore::particle_set_kind(
//!   index, kind)` (Phase I)
//! * colour swatches — `EditorCore::particle_set_color(
//!   index, color)` (Phase I)
//! * anchor — `EditorCore::particle_set_anchor(index,
//!   figure, node)` with `Option` figure/node, mirroring
//!   the model's `ParticleEvent { figure: Option<usize>,
//!   node: Option<usize>, .. }` (model.rs:602): `None`
//!   figure = a free stage point at the event's stored
//!   `x`/`y`.
//!
//! `particle_set_position` exists in the same op file and
//! the model supports it (the event carries `x`/`y`), but
//! this panel deliberately offers no position control:
//! a free event's point is set by import or by the stage,
//! and the detail block shows it read-only. Selection
//! bookkeeping stays in this module (the ops know nothing
//! of `State::particle_selected`): add selects the new
//! event, delete repairs the selection with
//! [`selection_after_delete`].

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{ParticleEvent, ParticleKind};

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
use crate::State;

// ----------------------------------------------------------------
// Pure data helpers
// ----------------------------------------------------------------

/// Snake-case kind name (the export/MCP vocabulary for the
/// three `ParticleKind`s).
pub fn kind_name(kind: ParticleKind) -> &'static str {
    match kind {
        ParticleKind::Burst => "burst",
        ParticleKind::Sparkle => "sparkle",
        ParticleKind::Smoke => "smoke",
    }
}

/// Every kind, in the `ParticleKind` enum's declaration
/// order (model.rs: Burst, Sparkle, Smoke) — the order
/// the detail block's kind stepper cycles through.
pub const KIND_ORDER: [ParticleKind; 3] = [
    ParticleKind::Burst,
    ParticleKind::Sparkle,
    ParticleKind::Smoke,
];

/// The kind one step from `current` in [`KIND_ORDER`],
/// wrapping at both ends (`dir` is +1 / −1 from the
/// stepper buttons; any other value still lands on a
/// kind, by the same modular step).
pub fn cycle_kind(current: ParticleKind, dir: i32) -> ParticleKind {
    let count = KIND_ORDER.len() as i32;
    let index = KIND_ORDER
        .iter()
        .position(|&kind| kind == current)
        .unwrap_or(0) as i32;
    KIND_ORDER[(index + dir).rem_euclid(count) as usize]
}

/// The detail block's colour swatches (`0xRRGGBB`): six
/// hues around the wheel, then white and black. The hues
/// reuse the shell's own accent vocabulary where it has
/// one (handle red, selection amber, onion green, the
/// secondary accent violet).
pub const PARTICLE_PALETTE: [u32; 8] = [
    0xFF5D5D, // red
    0xFF9F43, // orange
    0xFFC65D, // amber
    0x7BE495, // green
    0x57C7E3, // sky
    0xA184FF, // violet
    0xFFFFFF, // white
    0x000000, // black
];

/// One list row's title: `Event {i+1} · frame {f+1}`
/// (both numbers 1-based for display; the stored frame
/// is 0-based).
pub fn row_label(index: usize, event: &ParticleEvent) -> String {
    format!("Event {} · frame {}", index + 1, event.frame + 1)
}

/// Short anchor summary for a list row: `free` for a
/// stage-point event, `figure {n}` (1-based) for an
/// anchored one. The node, when present, is detail-block
/// information ([`anchor_detail`]), not row information.
pub fn anchor_summary(event: &ParticleEvent) -> String {
    match event.figure {
        None => "free".to_string(),
        Some(figure) => format!("figure {}", figure + 1),
    }
}

/// Full anchor description for the detail block: a free
/// event sits at its stage point; an anchored one names
/// the figure and, when a node is set, the node.
pub fn anchor_detail(event: &ParticleEvent) -> String {
    match (event.figure, event.node) {
        (None, _) => "free at stage point".to_string(),
        (Some(figure), None) => format!("anchored to figure {}", figure + 1),
        (Some(figure), Some(node)) => {
            format!("anchored to figure {} · node {}", figure + 1, node)
        }
    }
}

/// The selection if it still points at a live particle
/// event, else `None` (a stale index — e.g. after an undo
/// shrank the list — must never highlight a row or open
/// the detail block on the wrong event).
pub fn selected_event(core: &EditorCore, selected: Option<usize>) -> Option<usize> {
    selected.filter(|&i| i < core.project.particles.len())
}

/// One list row's data, built from the project's particle
/// events in list order.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleRowData {
    /// Position in `Project::particles`.
    pub index: usize,
    /// `Event {i+1} · frame {f+1}` title.
    pub label: String,
    /// `free` / `figure {n}` anchor summary.
    pub anchor: String,
    /// Whether this row is the detail block's event.
    pub selected: bool,
}

/// List rows for every particle event in the project.
/// `selected` is the raw view-state selection; it is
/// validated here so a stale index selects nothing.
pub fn event_rows(core: &EditorCore, selected: Option<usize>) -> Vec<ParticleRowData> {
    let selected = selected_event(core, selected);
    core.project
        .particles
        .iter()
        .enumerate()
        .map(|(index, event)| ParticleRowData {
            index,
            label: row_label(index, event),
            anchor: anchor_summary(event),
            selected: selected == Some(index),
        })
        .collect()
}

/// Everything the detail block shows/edits for one event.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleDetailData {
    /// Position in `Project::particles`.
    pub index: usize,
    /// Snake-case kind name.
    pub kind: &'static str,
    /// The kind itself, for the kind stepper's cycle.
    pub kind_value: ParticleKind,
    /// The event's colour (`0xRRGGBB`), for the swatch
    /// row's current-colour marker.
    pub color: u32,
    /// 0-based start frame (display adds 1).
    pub frame: usize,
    /// Full anchor description ([`anchor_detail`]).
    pub anchor: String,
    /// Anchored figure, if any (0-based).
    pub figure: Option<usize>,
    /// Stored stage point (the event's position when
    /// free; the anchor offset reference when anchored).
    pub x: f32,
    pub y: f32,
}

/// Detail data for the selected event, or `None` when
/// nothing (valid) is selected.
pub fn detail_data(core: &EditorCore, selected: Option<usize>) -> Option<ParticleDetailData> {
    let index = selected_event(core, selected)?;
    let event = &core.project.particles[index];
    Some(ParticleDetailData {
        index,
        kind: kind_name(event.kind),
        kind_value: event.kind,
        color: event.color,
        frame: event.frame,
        anchor: anchor_detail(event),
        figure: event.figure,
        x: event.x,
        y: event.y,
    })
}

/// Frame after a ±1 stepper press, clamped at frame 0
/// (frames are `usize`; there is no upper clamp — an
/// event may sit past the current timeline end, as
/// imported ones can).
pub fn stepped_frame(frame: usize, delta: i32) -> usize {
    (frame as i64 + delta as i64).max(0) as usize
}

/// Selection after event `deleted` was removed, with
/// `len_after` events remaining: a selection before the
/// gap is untouched, one after it shifts down by one,
/// and the deleted event's own selection lands on the
/// event that slid into its slot (or the new last event,
/// or `None` when the list is now empty). Same repair
/// the FX clips lane performs.
pub fn selection_after_delete(
    selected: Option<usize>,
    deleted: usize,
    len_after: usize,
) -> Option<usize> {
    let sel = selected?;
    if len_after == 0 {
        return None;
    }
    if sel < deleted {
        Some(sel)
    } else if sel > deleted {
        Some(sel - 1)
    } else {
        Some(deleted.min(len_after - 1))
    }
}

// ----------------------------------------------------------------
// Handlers (thin wrappers over the EditorCore particle ops
// in particle_ops.rs; the ops own the snapshot, these own
// the selection)
// ----------------------------------------------------------------

/// Add an event at the current frame, at the stage
/// centre, via `EditorCore::particle_add_event`, and
/// select it — the new event is the one the user most
/// likely wants to adjust next. Selection is only taken
/// when the list actually grew by one.
fn add_event(s: &mut State) {
    let frame = s.core.current;
    let [stage_w, stage_h] = s.core.project.stage;
    let len_before = s.core.project.particles.len();
    s.core
        .particle_add_event(frame, stage_w / 2.0, stage_h / 2.0);
    if s.core.project.particles.len() == len_before + 1 {
        s.particle_selected = Some(len_before);
    }
}

/// Delete the selected event via
/// `EditorCore::particle_delete_event` and repair the
/// selection with [`selection_after_delete`]. The op
/// returns nothing, so whether an event was actually
/// removed is judged by the list length.
fn delete_selected(s: &mut State) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    let len_before = s.core.project.particles.len();
    s.core.particle_delete_event(i);
    let len_after = s.core.project.particles.len();
    if len_after < len_before {
        s.particle_selected = selection_after_delete(s.particle_selected, i, len_after);
    }
}

/// Step the selected event's frame by `delta` via
/// `EditorCore::particle_set_frame`. A press that would
/// not move the frame (stepping earlier at frame 0)
/// calls nothing — the op would only take a snapshot.
fn step_frame(s: &mut State, delta: i32) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    let frame = s.core.project.particles[i].frame;
    let next = stepped_frame(frame, delta);
    if next != frame {
        s.core.particle_set_frame(i, next);
    }
}

/// Step the selected event's kind by `dir` through
/// [`cycle_kind`] via `EditorCore::particle_set_kind`.
/// Cycling always lands on a different kind (three kinds,
/// ±1 steps), so every press is a real edit and takes
/// the op's snapshot.
fn cycle_selected_kind(s: &mut State, dir: i32) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    let next = cycle_kind(s.core.project.particles[i].kind, dir);
    s.core.particle_set_kind(i, next);
}

/// Set the selected event's colour via
/// `EditorCore::particle_set_color` (a swatch press).
fn set_selected_color(s: &mut State, color: u32) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    s.core.particle_set_color(i, color);
}

/// Anchor the selected event to the core's selected
/// figure (and its selected joint, when one is chosen)
/// via `EditorCore::particle_set_anchor` — the same
/// selection the FX re-anchor reads. A core selection
/// past the project's figure list (possible after an
/// undo) anchors nothing and says so on the status line,
/// rather than anchoring to a figure that does not exist.
fn anchor_to_figure(s: &mut State) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    let figure = s.core.selected_figure;
    if figure >= s.core.project.figures.len() {
        s.core.status = "Select a figure first, then anchor the particle event.".into();
        return;
    }
    let node = s.core.selected_node;
    s.core.particle_set_anchor(i, Some(figure), node);
}

/// Clear the selected event's anchor via
/// `EditorCore::particle_set_anchor` with `None`/`None`:
/// the event becomes a free stage point at its stored
/// `x`/`y`.
fn anchor_clear(s: &mut State) {
    let Some(i) = selected_event(&s.core, s.particle_selected) else {
        return;
    };
    s.core.particle_set_anchor(i, None, None);
}

// ----------------------------------------------------------------
// Views
// ----------------------------------------------------------------

/// The detail block for the selected event: kind and
/// anchor summary, a kind stepper and colour swatches
/// (Phase I), a frame stepper, the anchor buttons
/// (anchor to the selected figure / clear anchor), and
/// delete. With events present but no valid selection it
/// renders a guidance line instead of dead buttons.
fn detail_block(s: &State, t: &Theme) -> El {
    let Some(data) = detail_data(&s.core, s.particle_selected) else {
        return col().key("particles-detail").w_full().child(
            text("Select a particle event to edit it.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    let i = data.index;
    // Colour swatches (the scene panel's construction): a
    // 26 pt slot per colour, accent-filled behind the
    // 22 pt colour chip when it is the event's colour.
    let accent = tok::accent(0, true);
    let mut colors = row().key("particles-colors").gap(4.0).center_y();
    for (ci, &rgb) in PARTICLE_PALETTE.iter().enumerate() {
        colors = colors.child(
            el()
                .key(format!("particles-color-{ci}"))
                .size(26.0, 26.0)
                .radius(8.0)
                .fill(if data.color == rgb { accent } else { t.bg })
                .center()
                .on_press(move |s: &mut State| set_selected_color(s, rgb))
                .child(
                    el()
                        .size(22.0, 22.0)
                        .radius(6.0)
                        .fill(crate::stage_view::hex_color(rgb).unwrap_or(t.bg)),
                ),
        );
    }
    col()
        .key("particles-detail")
        .w_full()
        .gap(8.0)
        .child(
            text(format!("Event {} · {}", i + 1, data.kind))
                .font(type_::TITLE)
                .color(t.text),
        )
        .child(
            text(format!(
                "{} · at {:.0}, {:.0}",
                data.anchor, data.x, data.y
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        // Kind stepper: chevrons cycle Burst → Sparkle →
        // Smoke through cycle_kind; the label names the
        // kind the event has now (EditorCore::
        // particle_set_kind).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Kind")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(icon_only_btn(
                    "particles-kind-prev",
                    "chevron-left",
                    26.0,
                    false,
                    t,
                    |s| cycle_selected_kind(s, -1),
                ))
                .child(
                    text(kind_name(data.kind_value))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(icon_only_btn(
                    "particles-kind-next",
                    "chevron-right",
                    26.0,
                    false,
                    t,
                    |s| cycle_selected_kind(s, 1),
                )),
        )
        // Colour swatches (built above): a press sets the
        // event's colour (EditorCore::particle_set_color).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Colour")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(colors),
        )
        // Frame stepper: -/+ move the event by one frame
        // (EditorCore::particle_set_frame).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Frame")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn("particles-frame-dn", "-", false, t, |s| {
                    step_frame(s, -1)
                }))
                .child(
                    text(format!("{}", data.frame + 1))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(btn("particles-frame-up", "+", false, t, |s| {
                    step_frame(s, 1)
                })),
        )
        // Anchor: attach to the core's selected figure,
        // or clear back to a free stage point.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon_btn(
                    "particles-anchor-fig",
                    "crosshairs",
                    "Anchor to figure",
                    false,
                    t,
                    |s| anchor_to_figure(s),
                ))
                .child(icon_btn(
                    "particles-anchor-clear",
                    "xmark",
                    "Clear anchor",
                    false,
                    t,
                    |s| anchor_clear(s),
                )),
        )
        .child(
            row().gap(8.0).center_y().child(icon_btn(
                "particles-delete",
                "trash-can",
                "Delete event",
                false,
                t,
                |s| delete_selected(s),
            )),
        )
}

/// The particle events panel: header + count, an add
/// button, one selectable row per `Project::particles`
/// event (the real vec index, so the stage gizmo and the
/// ops agree on which event is meant), and the detail
/// block for the selection. Particle events mostly
/// arrive with imports (.stk / Stick Nodes projects
/// carry them); the add button exists so a project can
/// gain one without an import.
pub fn particles_panel(s: &State, t: &Theme) -> El {
    let rows = event_rows(&s.core, s.particle_selected);
    let accent = tok::accent(0, true);
    let mut list = col().key("particles-list").gap(4.0);
    if rows.is_empty() {
        list = list.child(
            text("No particle events yet — they arrive with imports, or add one.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for data in rows {
        let i = data.index;
        // Leading star in the event's own colour (the
        // project event at this row's index carries it).
        let color = s
            .core
            .project
            .particles
            .get(i)
            .and_then(|e| crate::stage_view::hex_color(e.color))
            .unwrap_or(t.text_secondary);
        list = list.child(
            row()
                .key(format!("particle-row-{i}"))
                .w_full()
                .gap(8.0)
                .center_y()
                .px(8.0)
                .py(5.0)
                .radius(6.0)
                .fill(if data.selected {
                    t.control_track
                } else {
                    t.card
                })
                .hover(if data.selected {
                    t.control_track
                } else {
                    t.control_off
                })
                .on_press(move |s: &mut State| {
                    s.particle_selected = Some(i);
                })
                .child(icon_el("star", 12.0, color))
                .child(
                    text(data.label)
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    text(data.anchor)
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(if data.selected {
                    el().size(8.0, 8.0).radius(4.0).fill(accent)
                } else {
                    el().size(8.0, 8.0)
                }),
        );
    }
    let mut panel = col()
        .key("particles")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("Particles").font(type_::TITLE).color(t.text))
                .child(
                    text(format!("{} event(s)", s.core.project.particles.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(
            row().gap(8.0).center_y().child(icon_btn(
                "particles-add",
                "star",
                "+ Event",
                false,
                t,
                |s| add_event(s),
            )),
        )
        .child(list);
    if !s.core.project.particles.is_empty() {
        panel = panel.child(detail_block(s, t));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(frame: usize, figure: Option<usize>, node: Option<usize>) -> ParticleEvent {
        ParticleEvent {
            frame,
            figure,
            node,
            x: 100.0,
            y: 50.0,
            kind: ParticleKind::Sparkle,
            color: 0xFFC65D,
            seed: 7,
        }
    }

    fn core_with(events: Vec<ParticleEvent>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.particles = events;
        core
    }

    fn demo_state() -> State {
        State {
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
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: None,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            was_playing: false,
        }
    }

    #[test]
    fn row_label_is_one_based_event_and_frame() {
        assert_eq!(row_label(0, &event(0, None, None)), "Event 1 · frame 1");
        assert_eq!(row_label(2, &event(4, None, None)), "Event 3 · frame 5");
        assert_eq!(row_label(9, &event(29, None, None)), "Event 10 · frame 30");
    }

    #[test]
    fn anchor_summary_names_free_or_the_figure() {
        assert_eq!(anchor_summary(&event(0, None, None)), "free");
        // A stray node without a figure still reads free:
        // the anchor is the figure.
        assert_eq!(anchor_summary(&event(0, None, Some(2))), "free");
        assert_eq!(anchor_summary(&event(0, Some(0), None)), "figure 1");
        assert_eq!(anchor_summary(&event(0, Some(1), Some(3))), "figure 2");
    }

    #[test]
    fn anchor_detail_adds_the_node_when_present() {
        assert_eq!(anchor_detail(&event(0, None, None)), "free at stage point");
        assert_eq!(
            anchor_detail(&event(0, Some(0), None)),
            "anchored to figure 1"
        );
        assert_eq!(
            anchor_detail(&event(0, Some(1), Some(3))),
            "anchored to figure 2 · node 3"
        );
    }

    #[test]
    fn kind_name_covers_every_kind() {
        assert_eq!(kind_name(ParticleKind::Burst), "burst");
        assert_eq!(kind_name(ParticleKind::Sparkle), "sparkle");
        assert_eq!(kind_name(ParticleKind::Smoke), "smoke");
    }

    #[test]
    fn selected_event_clamps_stale_selections() {
        let core = core_with(vec![event(0, None, None), event(3, None, None)]);
        assert_eq!(selected_event(&core, None), None);
        assert_eq!(selected_event(&core, Some(0)), Some(0));
        assert_eq!(selected_event(&core, Some(1)), Some(1));
        // Stale: past the end selects nothing.
        assert_eq!(selected_event(&core, Some(2)), None);
        assert_eq!(selected_event(&core, Some(99)), None);
        // Empty project: every selection is stale.
        assert_eq!(selected_event(&core_with(vec![]), Some(0)), None);
    }

    #[test]
    fn event_rows_carry_label_anchor_and_selection() {
        let core = core_with(vec![
            event(0, None, None),
            event(4, Some(1), Some(2)),
        ]);
        let rows = event_rows(&core, Some(1));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Event 1 · frame 1");
        assert_eq!(rows[0].anchor, "free");
        assert!(!rows[0].selected);
        assert_eq!(rows[1].label, "Event 2 · frame 5");
        assert_eq!(rows[1].anchor, "figure 2");
        assert!(rows[1].selected);
        // Empty project rows nothing; stale selection
        // selects nothing.
        assert!(event_rows(&core_with(vec![]), Some(0)).is_empty());
        assert!(event_rows(&core, Some(9)).iter().all(|r| !r.selected));
    }

    #[test]
    fn detail_data_needs_a_valid_selection() {
        let core = core_with(vec![event(4, Some(0), Some(3))]);
        assert!(detail_data(&core, None).is_none());
        assert!(detail_data(&core, Some(1)).is_none(), "out of range");
        let data = detail_data(&core, Some(0)).expect("valid selection");
        assert_eq!(data.index, 0);
        assert_eq!(data.kind, "sparkle");
        assert_eq!(data.frame, 4);
        assert_eq!(data.anchor, "anchored to figure 1 · node 3");
        assert_eq!(data.figure, Some(0));
        assert_eq!(data.x, 100.0);
        assert_eq!(data.y, 50.0);
    }

    #[test]
    fn stepped_frame_clamps_at_zero_only() {
        assert_eq!(stepped_frame(3, -1), 2);
        assert_eq!(stepped_frame(0, -1), 0);
        assert_eq!(stepped_frame(0, 1), 1);
        assert_eq!(stepped_frame(41, 1), 42);
    }

    #[test]
    fn selection_after_delete_repairs_each_position() {
        // Nothing selected stays nothing.
        assert_eq!(selection_after_delete(None, 0, 3), None);
        // Empty list clears the selection.
        assert_eq!(selection_after_delete(Some(0), 0, 0), None);
        // Before the gap: untouched.
        assert_eq!(selection_after_delete(Some(0), 2, 4), Some(0));
        // After the gap: shifts down one.
        assert_eq!(selection_after_delete(Some(3), 1, 4), Some(2));
        // The deleted event itself: the event that slid
        // in, or the new last event when the tail went.
        assert_eq!(selection_after_delete(Some(1), 1, 4), Some(1));
        assert_eq!(selection_after_delete(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn add_event_lands_at_current_frame_stage_centre_and_selects() {
        let mut state = demo_state();
        state.core.current = 2;
        let [w, h] = state.core.project.stage;
        add_event(&mut state);
        assert_eq!(state.core.project.particles.len(), 1);
        let added = &state.core.project.particles[0];
        assert_eq!(added.frame, 2, "the current frame");
        assert_eq!(added.x, w / 2.0, "stage centre x");
        assert_eq!(added.y, h / 2.0, "stage centre y");
        assert_eq!(state.particle_selected, Some(0), "new event selected");
        // A second add appends and selects the new tail.
        add_event(&mut state);
        assert_eq!(state.core.project.particles.len(), 2);
        assert_eq!(state.particle_selected, Some(1));
    }

    #[test]
    fn step_frame_moves_the_selected_event_and_clamps() {
        let mut state = demo_state();
        state.core.project.particles = vec![event(1, None, None)];
        state.particle_selected = Some(0);
        step_frame(&mut state, 1);
        assert_eq!(state.core.project.particles[0].frame, 2);
        step_frame(&mut state, -1);
        step_frame(&mut state, -1);
        assert_eq!(state.core.project.particles[0].frame, 0);
        // Stepping earlier at frame 0 is a no-op.
        step_frame(&mut state, -1);
        assert_eq!(state.core.project.particles[0].frame, 0);
        // No selection: nothing moves, no panic.
        state.particle_selected = None;
        step_frame(&mut state, 1);
        assert_eq!(state.core.project.particles[0].frame, 0);
    }

    #[test]
    fn anchor_handlers_use_the_core_selection_and_clear() {
        let mut state = demo_state();
        state.core.project.particles = vec![event(0, None, None)];
        state.particle_selected = Some(0);
        // The demo project has figures and the core
        // selection starts on figure 0.
        let figure = state.core.selected_figure;
        assert!(figure < state.core.project.figures.len());
        anchor_to_figure(&mut state);
        assert_eq!(state.core.project.particles[0].figure, Some(figure));
        anchor_clear(&mut state);
        assert_eq!(state.core.project.particles[0].figure, None);
        assert_eq!(state.core.project.particles[0].node, None);
    }

    #[test]
    fn delete_selected_removes_and_repairs_the_selection() {
        let mut state = demo_state();
        state.core.project.particles = vec![
            event(0, None, None),
            event(2, None, None),
            event(4, None, None),
        ];
        state.particle_selected = Some(2);
        delete_selected(&mut state);
        assert_eq!(state.core.project.particles.len(), 2);
        // The deleted tail's selection lands on the new
        // last event.
        assert_eq!(state.particle_selected, Some(1));
        // Deleting the last remaining events clears it.
        delete_selected(&mut state);
        delete_selected(&mut state);
        assert!(state.core.project.particles.is_empty());
        assert_eq!(state.particle_selected, None);
    }

    #[test]
    fn cycle_kind_wraps_both_ends_and_covers_every_variant_once() {
        // Forward through the enum's declaration order.
        assert_eq!(cycle_kind(ParticleKind::Burst, 1), ParticleKind::Sparkle);
        assert_eq!(cycle_kind(ParticleKind::Sparkle, 1), ParticleKind::Smoke);
        // Forward wrap: the last kind cycles to the first.
        assert_eq!(cycle_kind(ParticleKind::Smoke, 1), ParticleKind::Burst);
        // Backward wrap: the first kind cycles to the last.
        assert_eq!(cycle_kind(ParticleKind::Burst, -1), ParticleKind::Smoke);
        assert_eq!(cycle_kind(ParticleKind::Smoke, -1), ParticleKind::Sparkle);
        assert_eq!(cycle_kind(ParticleKind::Sparkle, -1), ParticleKind::Burst);
        // A full forward lap visits every variant exactly
        // once and returns to the start.
        let mut lap = vec![ParticleKind::Burst];
        let mut current = ParticleKind::Burst;
        for _ in 1..KIND_ORDER.len() {
            current = cycle_kind(current, 1);
            lap.push(current);
        }
        for kind in KIND_ORDER {
            assert_eq!(
                lap.iter().filter(|&&k| k == kind).count(),
                1,
                "{kind:?} visited exactly once per lap"
            );
        }
        assert_eq!(cycle_kind(current, 1), ParticleKind::Burst);
    }

    #[test]
    fn particle_palette_has_eight_distinct_colours() {
        assert_eq!(PARTICLE_PALETTE.len(), 8);
        let mut sorted = PARTICLE_PALETTE;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "every swatch colour is distinct");
        assert!(PARTICLE_PALETTE.contains(&0xFFFFFF), "white is in");
        assert!(PARTICLE_PALETTE.contains(&0x000000), "black is in");
    }

    #[test]
    fn detail_data_carries_the_kind_value_and_color() {
        let core = core_with(vec![event(4, Some(0), Some(3))]);
        let data = detail_data(&core, Some(0)).expect("valid selection");
        assert_eq!(data.kind_value, ParticleKind::Sparkle);
        assert_eq!(data.color, 0xFFC65D);
    }

    #[test]
    fn cycle_selected_kind_steps_and_wraps_the_selected_event() {
        let mut state = demo_state();
        // The fixture event is a Sparkle.
        state.core.project.particles = vec![event(0, None, None)];
        state.particle_selected = Some(0);
        cycle_selected_kind(&mut state, 1);
        assert_eq!(state.core.project.particles[0].kind, ParticleKind::Smoke);
        cycle_selected_kind(&mut state, 1);
        assert_eq!(
            state.core.project.particles[0].kind,
            ParticleKind::Burst,
            "wrapped forward"
        );
        cycle_selected_kind(&mut state, -1);
        assert_eq!(
            state.core.project.particles[0].kind,
            ParticleKind::Smoke,
            "wrapped backward"
        );
        // No selection: nothing changes, no panic.
        state.particle_selected = None;
        cycle_selected_kind(&mut state, 1);
        assert_eq!(state.core.project.particles[0].kind, ParticleKind::Smoke);
    }

    #[test]
    fn set_selected_color_writes_only_the_selected_event() {
        let mut state = demo_state();
        state.core.project.particles = vec![event(0, None, None), event(2, None, None)];
        state.particle_selected = Some(1);
        set_selected_color(&mut state, PARTICLE_PALETTE[0]);
        assert_eq!(state.core.project.particles[1].color, PARTICLE_PALETTE[0]);
        assert_eq!(
            state.core.project.particles[0].color, 0xFFC65D,
            "the other event keeps its colour"
        );
        // No selection: nothing changes, no panic.
        state.particle_selected = None;
        set_selected_color(&mut state, PARTICLE_PALETTE[3]);
        assert_eq!(state.core.project.particles[1].color, PARTICLE_PALETTE[0]);
    }

    #[test]
    fn panel_builds_for_empty_selected_and_stale_states() {
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        let _ = particles_panel(&state, &t);
        state.core.project.particles = vec![event(0, None, None), event(3, Some(0), None)];
        state.particle_selected = Some(1);
        let _ = particles_panel(&state, &t);
        // A stale selection builds the guidance block,
        // not a detail block on the wrong event.
        state.particle_selected = Some(9);
        let _ = particles_panel(&state, &t);
    }
}
