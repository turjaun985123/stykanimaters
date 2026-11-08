//! FX clips lane + clip inspector for the Rune editor shell
//! (Phase D, section 14; Phase E, section 16 — handlers
//! moved onto the real EditorCore FX ops). Declared from
//! `main.rs` only — this is bin-crate code, not part of the
//! engine library.
//!
//! ## Integration contract
//!
//! This section needs ONE field on `State` in `main.rs`:
//!
//! ```ignore
//! pub fx_selected: Option<usize>,
//! ```
//!
//! Selection is pure view state: there is no EditorCore FX
//! selection op or field (the `ClipEditSession` in
//! `editor.rs` is the *movieclip* session, unrelated), and
//! the GPUI FX panel likewise kept no core-side FX selection
//! — it rendered one row per clip straight from
//! `fx_edit::list`. Here a row press records the clip index
//! in `State::fx_selected` and the inspector edits that clip.
//!
//! ## Ops (all real; no invented engine API)
//!
//! Every mutation goes through the `EditorCore` FX ops in
//! `src/fx_ops.rs` (Phase E, section 9), which own the undo
//! snapshot and route to the `fx_edit` backend the MCP
//! tools also use:
//!
//! * earlier / later + start stepper —
//!   `EditorCore::fx_move_clip(index, delta)` (whole-frame
//!   delta, `i32` — the codebase's move-op convention, cf.
//!   `shift_frames` / `move_figure`; the backend clamps the
//!   resulting start to >= 0 and fractional starts keep
//!   their fraction)
//! * duration stepper — `EditorCore::fx_resize_clip(index,
//!   delta)` (frame delta, `f32`; the backend clamps the
//!   resulting duration to >= 0.1)
//! * duplicate — `EditorCore::fx_duplicate_clip(index)`,
//!   returning the copy's index (the copy is inserted right
//!   after the original, in list and in time)
//! * delete — `EditorCore::fx_delete_clip(index)`
//! * colour / end colour — `EditorCore::fx_set_color` /
//!   `fx_set_color2`
//! * re-anchor — `EditorCore::fx_set_anchor(index, figure,
//!   node)`, the anchoring `add_fx_clip` performs at
//!   creation (editor.rs:2005)
//!
//! Phase D note: this section originally snapshotted
//! locally and called `fx_edit` (or wrote colour/anchor
//! fields) directly, because no EditorCore FX ops existed;
//! Phase E replaced those handlers with the ops above.
//! Selection bookkeeping stays in this module (the ops
//! know nothing of `State::fx_selected`): duplicate selects
//! the returned index, delete repairs the selection with
//! [`selection_after_delete`]. The one status line the
//! handlers still own is the no-joint re-anchor message —
//! `fx_set_anchor` assumes a valid joint, so the handler
//! checks `selected_node` first, exactly as `add_fx_clip`
//! does.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El, ScrollSpec};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fx_edit;

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Swatch colours offered for clip colour / end colour: the
/// 16-colour picker palette the GPUI colour panel used
/// (pre-removal `ui/color.rs` `PICKER_PALETTE`), so the Rune
/// shell offers the same choices the old UI did.
pub const SWATCHES: [u32; 16] = [
    0xFFFFFF, 0x9AA3B2, 0x232830, 0x000000, 0xFF5D5D, 0xFF9F43, 0xFFC65D, 0xFFE066, 0x7BE495,
    0x2EC4B6, 0x4DD0E1, 0x5DB2FF, 0x3D5A99, 0xC792FF, 0xFF7BAC, 0x8D6E63,
];

// ----------------------------------------------------------------
// Pure data helpers
// ----------------------------------------------------------------

/// Display text for a frame position: whole frames render
/// without a fraction (`4`), fractional ones with one
/// decimal (`4.5`). Clip starts/durations are `f32` (the
/// `fx_edit` move/resize ops accept fractional frames), so
/// the label must not silently truncate them.
pub fn frame_label(frame: f32) -> String {
    if frame.fract() == 0.0 {
        format!("{}", frame as i64)
    } else {
        format!("{frame:.1}")
    }
}

/// The 1-based frame range a clip covers, e.g. `frames
/// 5–14` for the half-open window `[4, 14)`: the first
/// covered frame is `start + 1` and the last is `end`
/// (the same convention the GPUI panel's `f{start+1}`
/// label used).
pub fn window_text(start: f32, end: f32) -> String {
    format!(
        "frames {}–{}",
        frame_label(start + 1.0),
        frame_label(end)
    )
}

/// One lane row's data, built from `fx_edit::list` plus the
/// entry's colours (which `FxInfo` does not carry).
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRowData {
    /// Position in `Project::fx_clips`.
    pub index: usize,
    /// Snake-case kind name (MCP vocabulary, e.g. `burst`).
    pub kind: &'static str,
    /// Row title: `{kind} #{index+1}` (clips have no name
    /// field of their own).
    pub name: String,
    /// `frames {first}–{last}` coverage text.
    pub window: String,
    /// `fig {figure+1} · node {node}` anchor text.
    pub anchor: String,
    /// Clip start frame (fractional-capable).
    pub start: f32,
    /// Clip length in frames.
    pub duration: f32,
    /// Primary colour, `0xRRGGBB`.
    pub color: u32,
    /// Gradient end colour, if any.
    pub color2: Option<u32>,
    /// Whether this row is the inspector's clip.
    pub selected: bool,
}

/// The selection if it still points at a live clip, else
/// `None` (a stale index — e.g. after an undo shrank the
/// list — must never highlight a row or open an inspector
/// on the wrong clip).
pub fn valid_selection(selected: Option<usize>, clip_count: usize) -> Option<usize> {
    selected.filter(|&i| i < clip_count)
}

/// Lane rows for every FX clip in the project, in list
/// order. `selected` is the raw view-state selection; it
/// is validated here so a stale index selects nothing.
pub fn clip_rows(core: &EditorCore, selected: Option<usize>) -> Vec<ClipRowData> {
    let selected = valid_selection(selected, core.project.fx_clips.len());
    fx_edit::list(&core.project)
        .into_iter()
        .map(|info| {
            let entry = &core.project.fx_clips[info.index];
            ClipRowData {
                index: info.index,
                kind: info.kind,
                name: format!("{} #{}", info.kind, info.index + 1),
                window: window_text(info.start, info.end),
                anchor: format!("fig {} · node {}", info.figure + 1, info.node),
                start: info.start,
                duration: info.duration,
                color: entry.clip.color,
                color2: entry.clip.color2,
                selected: selected == Some(info.index),
            }
        })
        .collect()
}

/// Everything the inspector shows/edits for one clip.
#[derive(Clone, Debug, PartialEq)]
pub struct FxInspectorData {
    /// Position in `Project::fx_clips`.
    pub index: usize,
    /// Snake-case kind name.
    pub kind: &'static str,
    /// First frame the clip covers.
    pub start: f32,
    /// Length of the clip in frames.
    pub duration: f32,
    /// `frames {first}–{last}` coverage text.
    pub window: String,
    /// Primary colour, `0xRRGGBB`.
    pub color: u32,
    /// Gradient end colour (`None` = solid colour).
    pub color2: Option<u32>,
    /// Anchored figure (0-based; display adds 1).
    pub figure: usize,
    /// Anchored node of that figure.
    pub node: usize,
}

/// Inspector data for the selected clip, or `None` when
/// nothing (valid) is selected.
pub fn inspector_data(core: &EditorCore, selected: Option<usize>) -> Option<FxInspectorData> {
    let index = valid_selection(selected, core.project.fx_clips.len())?;
    let info = &fx_edit::list(&core.project)[index];
    let entry = &core.project.fx_clips[index];
    Some(FxInspectorData {
        index,
        kind: info.kind,
        start: info.start,
        duration: info.duration,
        window: window_text(info.start, info.end),
        color: entry.clip.color,
        color2: entry.clip.color2,
        figure: entry.figure,
        node: entry.node,
    })
}

/// Selection after clip `deleted` was removed, with
/// `len_after` clips remaining: a selection before the gap
/// is untouched, one after it shifts down by one, and the
/// deleted clip's own selection lands on the clip that
/// slid into its slot (or the new last clip, or `None`
/// when the list is now empty).
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
// Handlers (thin wrappers over the EditorCore FX ops in
// fx_ops.rs; the ops own the snapshot, these own the
// selection)
// ----------------------------------------------------------------

/// Nudge clip `i` by `delta` whole frames via
/// `EditorCore::fx_move_clip`. Returns false (and calls
/// nothing) when `i` is out of range, or when moving
/// earlier from frame 0 — a no-op the op would only
/// snapshot (the GPUI panel's `fx-earlier` guard, kept).
fn nudge_clip_start(core: &mut EditorCore, i: usize, delta: i32) -> bool {
    let Some(entry) = core.project.fx_clips.get(i) else {
        return false;
    };
    if delta < 0 && entry.clip.start <= 0.0 {
        return false;
    }
    core.fx_move_clip(i, delta);
    true
}

/// Nudge the selected clip's start by `delta` frames.
fn nudge_start(s: &mut State, delta: i32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    nudge_clip_start(&mut s.core, i, delta);
}

/// Nudge the selected clip's duration by `delta` frames
/// via `EditorCore::fx_resize_clip` (the op applies the
/// delta itself and the backend clamps the result to
/// >= 0.1, so the handler passes the delta through
/// untouched).
fn nudge_duration(s: &mut State, delta: f32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_resize_clip(i, delta);
}

/// Duplicate the selected clip via
/// `EditorCore::fx_duplicate_clip`; the copy becomes the
/// selection, as it is the clip the user most likely
/// wants to adjust next.
fn duplicate_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    if let Some(new_index) = s.core.fx_duplicate_clip(i) {
        s.fx_selected = Some(new_index);
    }
}

/// Delete the selected clip via
/// `EditorCore::fx_delete_clip` and repair the selection
/// with [`selection_after_delete`]. The op returns
/// nothing, so whether a clip was actually removed is
/// judged by the list length.
fn delete_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let len_before = s.core.project.fx_clips.len();
    s.core.fx_delete_clip(i);
    let len_after = s.core.project.fx_clips.len();
    if len_after < len_before {
        s.fx_selected = selection_after_delete(s.fx_selected, i, len_after);
    }
}

/// Set the selected clip's primary colour via
/// `EditorCore::fx_set_color`.
fn set_color(s: &mut State, color: u32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_set_color(i, color);
}

/// Set (or clear, with `None`) the selected clip's
/// gradient end colour via `EditorCore::fx_set_color2`.
fn set_color2(s: &mut State, color2: Option<u32>) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_set_color2(i, color2);
}

/// Re-anchor the selected clip to the currently selected
/// joint (`EditorCore::selected_figure` /
/// `selected_node`) via `EditorCore::fx_set_anchor` — the
/// anchoring `add_fx_clip` performs at creation. The op
/// bounds-checks only the clip index and assumes a valid
/// joint, so the no-joint case is handled here with the
/// same status message `add_fx_clip` uses; on success the
/// op's own status line stands (it kept this panel's
/// exact "FX clip N re-anchored to fig F node M."
/// wording).
fn reanchor_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let Some(node) = s.core.selected_node else {
        s.core.status = "Select a joint first, then re-anchor the FX clip.".into();
        return;
    };
    let figure = s.core.selected_figure;
    s.core.fx_set_anchor(i, figure, node);
}

// ----------------------------------------------------------------
// Views
// ----------------------------------------------------------------

/// One colour swatch: a rounded square filled with the
/// colour; the current value carries a white outline
/// ring. Pressing it runs `f` (a colour-set handler).
fn swatch(key: String, hex: u32, current: bool, f: impl Fn(&mut State) + 'static) -> El {
    let fill = hex_color(hex).unwrap_or(Color::WHITE);
    let swatch = el()
        .key(key)
        .size(18.0, 18.0)
        .radius(5.0)
        .fill(fill)
        .hover(fill)
        .on_press(f);
    if current {
        swatch.outline(2.0, 1.0, Color::WHITE)
    } else {
        swatch
    }
}

/// A labelled row of swatches (two rows of eight, so the
/// inspector stays narrow), built by `make` per colour so
/// the primary-colour and end-colour rows share the code
/// but not the element keys or handlers.
fn swatch_rows(
    clip_index: usize,
    key_stem: &str,
    current: Option<u32>,
    mut make: impl FnMut(u32) -> Box<dyn Fn(&mut State)>,
) -> El {
    let mut rows = col().gap(4.0);
    for chunk in SWATCHES.chunks(8) {
        let mut row = row().gap(4.0);
        for &hex in chunk {
            let key = format!("{key_stem}-{}-{hex:06x}", clip_index);
            row = row.child(swatch(key, hex, current == Some(hex), make(hex)));
        }
        rows = rows.child(row);
    }
    rows
}

/// The clips lane: one row per FX clip in the project —
/// colour dot, kind name, covered frame range, anchor —
/// with the selected row highlighted. A row press selects
/// the clip for the inspector (`State::fx_selected`).
pub fn fx_lane(s: &State, t: &Theme) -> El {
    let rows = clip_rows(
        &s.core,
        valid_selection(s.fx_selected, s.core.project.fx_clips.len()),
    );
    let accent = tok::accent(0, true);
    let mut list = col().key("fx-clips").gap(4.0);
    if rows.is_empty() {
        list = list.child(
            text("No FX clips in this project.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for data in rows {
        let i = data.index;
        let dot_fill = hex_color(data.color).unwrap_or(Color::WHITE);
        list = list.child(
            row()
                .key(format!("fx-clip-{i}"))
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
                    s.fx_selected = Some(i);
                })
                .child(
                    el()
                        .size(10.0, 10.0)
                        .radius(5.0)
                        .fill(dot_fill),
                )
                .child(text(data.name).font(type_::LABEL).color(t.text))
                .child(
                    text(data.window)
                        .font(type_::LABEL)
                        .color(t.text_secondary)
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
    col()
        .key("fx-lane")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("FX clips").font(type_::TITLE).color(t.text))
                .child(
                    text(format!("{} clip(s)", s.core.project.fx_clips.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(list.scroll(ScrollSpec { stick_bottom: false }).min_h(0.0))
}

/// The clip inspector for the selected FX clip: kind and
/// anchor summary, start/duration steppers (move/resize),
/// earlier/later nudge buttons (the GPUI panel's
/// `fx-earlier`/`fx-later`), colour + end-colour swatch
/// rows, a re-anchor button, and duplicate/delete.
/// With no valid selection it shows a hint line instead.
pub fn fx_inspector(s: &State, t: &Theme) -> El {
    let Some(data) = inspector_data(&s.core, s.fx_selected) else {
        return col()
            .key("fx-inspector")
            .w_full()
            .child(
                text("Select an FX clip to edit it.")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
    };
    let i = data.index;
    let mut panel = col()
        .key("fx-inspector")
        .w_full()
        .gap(8.0)
        .child(
            text(format!("{} · clip {}", data.kind, i + 1))
                .font(type_::TITLE)
                .color(t.text),
        )
        .child(
            text(format!(
                "{} · anchor fig {} · node {}",
                data.window,
                data.figure + 1,
                data.node
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        // Start stepper: -/+ move the clip by one frame
        // (EditorCore::fx_move_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Start")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn(&format!("fx-start-dec-{i}"), "-", false, t, |s| {
                    nudge_start(s, -1)
                }))
                .child(
                    text(frame_label(data.start + 1.0))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(btn(&format!("fx-start-inc-{i}"), "+", false, t, |s| {
                    nudge_start(s, 1)
                })),
        )
        // Duration stepper: -/+ resize by one frame
        // (EditorCore::fx_resize_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Duration")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn(&format!("fx-dur-dec-{i}"), "-", false, t, |s| {
                    nudge_duration(s, -1.0)
                }))
                .child(
                    text(frame_label(data.duration))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(btn(&format!("fx-dur-inc-{i}"), "+", false, t, |s| {
                    nudge_duration(s, 1.0)
                })),
        )
        // Earlier / later: the GPUI panel's per-row nudge
        // buttons, same ops as the start stepper.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(&format!("fx-earlier-{i}"), "Earlier", false, t, |s| {
                    nudge_start(s, -1)
                }))
                .child(btn(&format!("fx-later-{i}"), "Later", false, t, |s| {
                    nudge_start(s, 1)
                }))
                .child(el().grow(1.0))
                .child(btn(&format!("fx-dup-{i}"), "Duplicate", false, t, |s| {
                    duplicate_selected(s)
                }))
                .child(btn(&format!("fx-del-{i}"), "Delete", false, t, |s| {
                    delete_selected(s)
                })),
        )
        // Colours: primary, then gradient end colour
        // (with a Solid chip clearing color2 to None).
        .child(
            text("Colour")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(swatch_rows(i, "fx-color", Some(data.color), |hex| {
            Box::new(move |s: &mut State| set_color(s, hex))
        }))
        .child(
            text("End colour")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(
                    &format!("fx-color2-none-{i}"),
                    "Solid",
                    data.color2.is_none(),
                    t,
                    |s| set_color2(s, None),
                ))
                .child(
                    text(if data.color2.is_none() {
                        "no gradient"
                    } else {
                        "gradient end"
                    })
                    .font(type_::LABEL)
                    .color(t.text_secondary),
                ),
        )
        .child(swatch_rows(i, "fx-color2", data.color2, |hex| {
            Box::new(move |s: &mut State| set_color2(s, Some(hex)))
        }))
        // Anchor: re-anchor to the selected joint.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(
                    &format!("fx-anchor-{i}"),
                    "Anchor to joint",
                    false,
                    t,
                    |s| reanchor_selected(s),
                ))
                .child(
                    text("uses the selected figure joint")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        );
    panel = panel.child(
        text(format!(
            "seed {} · angle {:.0}° · length {:.0}",
            s.core.project.fx_clips[i].clip.seed,
            s.core.project.fx_clips[i].clip.angle,
            s.core.project.fx_clips[i].clip.length
        ))
        .font(type_::LABEL)
        .color(t.text_secondary),
    );
    panel
}

/// Lane + inspector stacked — the whole section 14
/// surface, for the shell layout to mount as one block.
pub fn fx_section(s: &State, t: &Theme) -> El {
    col()
        .key("fx-section")
        .w_full()
        .gap(10.0)
        .child(fx_lane(s, t))
        .child(fx_inspector(s, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::FxClipEntry;
    use stykanimate_rs::vfx::{ClipKind, ParticleClip};

    fn entry(start: f32, duration: f32, kind: ClipKind, color: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind,
                color,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 1,
            node: 5,
        }
    }

    fn core_with(clips: Vec<FxClipEntry>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fx_clips = clips;
        core
    }

    #[test]
    fn frame_label_drops_integral_fractions_only() {
        assert_eq!(frame_label(4.0), "4");
        assert_eq!(frame_label(0.0), "0");
        assert_eq!(frame_label(4.5), "4.5");
        assert_eq!(frame_label(12.26), "12.3");
    }

    #[test]
    fn window_text_is_one_based_first_to_last_covered() {
        // [4, 14) covers frames 5..=14 in 1-based terms.
        assert_eq!(window_text(4.0, 14.0), "frames 5–14");
        assert_eq!(window_text(0.0, 16.0), "frames 1–16");
    }

    #[test]
    fn clip_rows_carry_kind_window_anchor_colour_and_selection() {
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF),
        ]);
        let rows = clip_rows(&core, Some(1));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "burst #1");
        assert_eq!(rows[0].window, "frames 5–14");
        assert_eq!(rows[0].anchor, "fig 2 · node 5");
        assert_eq!(rows[0].color, 0xFF5D5D);
        assert!(!rows[0].selected);
        assert_eq!(rows[1].kind, "shock_ring");
        assert!(rows[1].selected);
        // Empty project rows nothing; stale selection selects nothing.
        assert!(clip_rows(&core_with(vec![]), Some(0)).is_empty());
        assert!(clip_rows(&core, Some(9)).iter().all(|r| !r.selected));
    }

    #[test]
    fn inspector_data_needs_a_valid_selection() {
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFFC65D)]);
        assert!(inspector_data(&core, None).is_none());
        assert!(inspector_data(&core, Some(1)).is_none(), "out of range");
        let data = inspector_data(&core, Some(0)).expect("valid selection");
        assert_eq!(data.kind, "burst");
        assert_eq!(data.start, 4.0);
        assert_eq!(data.duration, 10.0);
        assert_eq!(data.window, "frames 5–14");
        assert_eq!(data.color, 0xFFC65D);
        assert_eq!(data.color2, None);
        assert_eq!(data.figure, 1);
        assert_eq!(data.node, 5);
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
        // The deleted clip itself: the clip that slid in,
        // or the new last clip when the tail was deleted.
        assert_eq!(selection_after_delete(Some(1), 1, 4), Some(1));
        assert_eq!(selection_after_delete(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn nudge_clip_start_moves_by_whole_frame_deltas() {
        let mut core = core_with(vec![entry(4.5, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        // The op takes a whole-frame delta; a fractional
        // start keeps its fraction.
        assert!(nudge_clip_start(&mut core, 0, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 5.5);
        assert!(nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 4.5);
        // A delta that would cross frame 0 is clamped by
        // the backend, and the call still counts as issued.
        let mut core = core_with(vec![entry(0.5, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
    }

    #[test]
    fn nudge_clip_start_guards_frame_zero_and_bad_index() {
        let mut core = core_with(vec![entry(0.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        // Earlier at frame 0: no call, no change (the op
        // would only take a snapshot).
        assert!(!nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
        // Out of range: no call, no panic.
        assert!(!nudge_clip_start(&mut core, 5, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0, "no change");
    }
}
