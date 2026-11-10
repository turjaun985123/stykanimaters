//! Figure operations promoted from the Rune shell into the
//! engine (Rune migration Phase E): setting a figure's colour
//! and duplicating a figure.
//!
//! Both began as dispatch helpers in the Phase D figure
//! inspector (`src/rune_sec/inspector_figure.rs`), because
//! the core offered no op for either. They live here now so
//! every surface — shell, MCP, scripts — shares one
//! implementation. The bookkeeping is the helpers', unchanged:
//!
//! * [`EditorCore::set_figure_color`] is a bounds-checked
//!   field write plus a status line — the same shape as
//!   `set_node_color` (editor.rs) and the MCP layer's
//!   figure-colour write. Wave 13: an actual colour change
//!   called `EditorCore::mark_dirty`, so the non-undoable
//!   write still counted toward the dirty dot and autosave
//!   cadence. Roadmap Wave 1 (RM24, P-14): a real change
//!   takes a coalesced undo step instead — the census of
//!   non-undoable writes is closed.
//! * [`EditorCore::duplicate_figure`] mirrors
//!   `delete_figure`'s bookkeeping (editor.rs) run in
//!   reverse, plus the index shifts an insertion requires.
//! * [`EditorCore::recolor_figure`] is the whole-figure
//!   counterpart of the per-node colour setters: one
//!   snapshot, then every node's colour written directly.
//! * [`EditorCore::mirror_figure_all_frames`] applies the
//!   transform `mirror_selected_pose` (editor.rs) performs
//!   on one frame — every angle negated, the root left in
//!   place — to the figure's pose on every frame, under
//!   one snapshot.
//!
//! Bookkeeping scope: duplication inserts cloned pose rows
//! (and bend rows, where a frame stores them) into every
//! frame, remaps every stored figure index in the project
//! (see [`remap_figures_after_insert`]), shifts the session's
//! solo target above the insertion point, and selects the
//! copy. The remap helpers below are the one deliberate pass
//! over the whole census of figure-index-bearing fields:
//!
//! * `Project::particles` — `ParticleEvent::figure: Option<usize>`;
//! * `Project::fx_clips` — `FxClipEntry::figure: usize`
//!   (model.rs; the anchor lives on the entry, not inside
//!   `vfx::ParticleClip`);
//! * `Project::channels` — `graph::Channel::figure: usize`;
//! * `Project::camera_follow: Option<usize>`.
//!
//! Fields checked and found NOT to carry a figure index:
//! `FigureInstance::clip` (a movieclip id), `ClipDef`
//! (self-contained def), `SpriteInstance::asset` (an asset
//! index), `TextInstance`, `AudioClip`, `Stroke`. Session
//! state is not the helpers' business: `EditorCore::solo`
//! is shifted by the ops themselves, and `selected_figure`
//! is set explicitly. `delete_figure` (editor.rs) wires the
//! removal helper with a one-line call — see
//! [`remap_figures_after_remove`].
//!
//! Reordering is covered by [`remap_figures_after_move`] and
//! its per-index core [`remap_index_after_move`]: a move
//! drops nothing, so every reference simply travels with
//! its figure to the figure's new index.

use crate::editor::EditorCore;
use crate::model::{Pose, Project};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Remap every stored figure index in `project` after a new
/// figure was inserted at `new_index` (the index the new
/// figure now occupies). References at or above `new_index`
/// named the figure that used to sit there — and everything
/// above it — so they shift up by one; references below it
/// are untouched. In particular a reference equal to the
/// *duplicated* figure's own index (`new_index - 1` for
/// `duplicate_figure`, which inserts the copy right after
/// the original) keeps naming the original.
pub fn remap_figures_after_insert(project: &mut Project, new_index: usize) {
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            if f >= new_index {
                e.figure = Some(f + 1);
            }
        }
    }
    for entry in &mut project.fx_clips {
        if entry.figure >= new_index {
            entry.figure += 1;
        }
    }
    for ch in &mut project.channels {
        if ch.figure >= new_index {
            ch.figure += 1;
        }
    }
    if let Some(f) = project.camera_follow {
        if f >= new_index {
            project.camera_follow = Some(f + 1);
        }
    }
}

/// Remap every stored figure index in `project` after the
/// figure at `removed` was deleted. References above
/// `removed` shift down by one. References TO the removed
/// figure are resolved per type, because their anchor is
/// gone:
///
/// * particle events anchored to it are dropped (the same
///   rule `delete_figure` already applied to particles);
/// * FX clip entries anchored to it are dropped —
///   `FxClipEntry::figure` is a plain `usize`, so there is
///   no "unanchored" value to fall back to, and a clip whose
///   anchor node no longer exists has no position to draw
///   at;
/// * graph channels targeting it are dropped — the channel
///   drove that figure's property and has nothing left to
///   drive (the graph evaluator tolerates stale channels,
///   but keeping dead ones would silently accumulate);
/// * `camera_follow` becomes `None` — following nothing is
///   the field's own "off" state.
///
/// Integrator note: `delete_figure` (editor.rs) gains the
/// full remap by replacing its `particles.retain(...)` block
/// with `crate::figure_ops::remap_figures_after_remove(&mut
/// self.project, fi);` after the figure/pose removal (plus
/// the session solo shift: `if self.solo == Some(fi) {
/// self.solo = None } else if let Some(s) = self.solo { if
/// s > fi { self.solo = Some(s - 1) } }`).
pub fn remap_figures_after_remove(project: &mut Project, removed: usize) {
    project.particles.retain(|e| e.figure != Some(removed));
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            if f > removed {
                e.figure = Some(f - 1);
            }
        }
    }
    project.fx_clips.retain(|entry| entry.figure != removed);
    for entry in &mut project.fx_clips {
        if entry.figure > removed {
            entry.figure -= 1;
        }
    }
    project.channels.retain(|ch| ch.figure != removed);
    for ch in &mut project.channels {
        if ch.figure > removed {
            ch.figure -= 1;
        }
    }
    match project.camera_follow {
        Some(f) if f == removed => project.camera_follow = None,
        Some(f) if f > removed => project.camera_follow = Some(f - 1),
        _ => {}
    }
}

/// The figure index a stored reference takes after the
/// figure at `from` is moved to `to` — the permutation a
/// remove-at-`from`, insert-at-`to` reorder applies to the
/// figures vec. A reference to the moved figure becomes
/// `to`; references strictly between `from` and `to` shift
/// by one toward `from` (those figures slide into the
/// vacated slot); every other reference is unchanged. An
/// adjacent swap is this same permutation with
/// `to == from ± 1`, so callers that swap neighbours use it
/// too. This is the same mapping `layers::remapped_index`
/// applies inside `layers::move_figure`, kept here so the
/// session state below and the project pass share one
/// definition.
///
/// Editor session state that stores a figure index —
/// `EditorCore::solo`, and a selection an op does not set
/// explicitly — is remapped by applying this fn to the
/// stored value; it takes plain indices because that state
/// lives on `EditorCore`, not on `Project`.
pub fn remap_index_after_move(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

/// Remap every stored figure index in `project` after the
/// figure at `from` was moved to `to` (the caller performs
/// the reorder itself — figures vec plus the parallel
/// per-frame pose/bend rows — this pass only fixes the
/// references). Unlike removal, nothing is dropped: every
/// anchor keeps its figure, just at the figure's new index,
/// via [`remap_index_after_move`]. `from == to` and indices
/// outside the figures vec are no-ops, mirroring the move
/// itself being refused there.
///
/// Integrator note: `EditorCore::move_figure` (editor.rs),
/// which currently swaps the figures/poses and remaps
/// nothing, gains the full remap with one call after its
/// swap — `crate::figure_ops::remap_figures_after_move(&mut
/// self.project, fi, target);` — plus the session fix-up
/// `if let Some(s) = self.solo { self.solo =
/// Some(crate::figure_ops::remap_index_after_move(s, fi,
/// target)); }` (its `selected_figure` is already assigned
/// `target` directly). `layers::move_figure` already
/// applies the same permutation internally, so
/// `move_figure_full` needs no change.
pub fn remap_figures_after_move(project: &mut Project, from: usize, to: usize) {
    let n = project.figures.len();
    if from == to || from >= n || to >= n {
        return;
    }
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            e.figure = Some(remap_index_after_move(f, from, to));
        }
    }
    for entry in &mut project.fx_clips {
        entry.figure = remap_index_after_move(entry.figure, from, to);
    }
    for ch in &mut project.channels {
        ch.figure = remap_index_after_move(ch.figure, from, to);
    }
    if let Some(f) = project.camera_follow {
        project.camera_follow = Some(remap_index_after_move(f, from, to));
    }
}

/// Mirror one pose in place, with exactly the transform
/// `EditorCore::mirror_selected_pose` (editor.rs) applies
/// to a single frame's pose: every joint angle is negated
/// and the root is left where it is. (An earlier guess
/// that the core op also mirrors the root across the
/// stage centre is wrong — its own test asserts the root
/// must not move.) Kept as a free helper so the
/// all-frames op below shares the one definition of the
/// transform instead of drifting from the core op.
fn mirror_pose_in_place(pose: &mut Pose) {
    for a in &mut pose.angles {
        *a = -*a;
    }
}

impl EditorCore {
    /// Set figure `fi`'s colour directly (the colour picker
    /// panels). Writes the same `FigureInstance.color` the
    /// add-figure ops assign from the `FIGURE_COLORS` cycle.
    /// Since Roadmap Wave 1 (RM24, P-14 — A15 H3) a write
    /// that actually changes the colour takes a coalesced
    /// undo step first, like `set_node_color` (Wave 13 only
    /// marked it dirty). Re-selecting the colour already
    /// stored changes nothing and spends nothing. An
    /// out-of-range `fi` only sets the status line.
    pub fn set_figure_color(&mut self, fi: usize, color: u32) {
        let changed = match self.project.figures.get(fi) {
            Some(fig) => fig.color != color,
            None => {
                self.status = "No such figure.".into();
                return;
            }
        };
        if changed {
            self.snapshot_coalesced("Figure colour");
            self.project.figures[fi].color = color;
        }
        self.status = format!("Figure {} colour changed.", fi + 1);
    }

    /// Duplicate figure `fi`: insert a clone of the instance
    /// right after the original, with the original's pose
    /// (and bend row, where one is stored) cloned into every
    /// frame at the same index, so the parallel per-frame
    /// tables stay parallel. Stored figure indices above the
    /// insertion point shift by one — particle anchors in
    /// the project, and the session's solo target — so they
    /// keep pointing at the same figure. The copy is named
    /// `"<name> copy"`, becomes the selection (with no node
    /// selected), and its index is returned. A movieclip
    /// figure's copy shares the same clip, like a second
    /// instance of it. Returns `None`, changing nothing but
    /// the status line, when `fi` names no figure.
    pub fn duplicate_figure(&mut self, fi: usize) -> Option<usize> {
        let Some(fig) = self.project.figures.get(fi).cloned() else {
            self.status = "No figure to duplicate.".into();
            return None;
        };
        // Clone the per-frame rows before mutating anything.
        let frame_poses: Vec<Option<Pose>> = self
            .project
            .frames
            .iter()
            .map(|f| f.poses.get(fi).cloned())
            .collect();
        let frame_bends: Vec<Option<Vec<f32>>> = self
            .project
            .frames
            .iter()
            .map(|f| f.bends.get(fi).cloned())
            .collect();
        self.snapshot();
        let mut copy = fig;
        copy.name = format!("{} copy", copy.name);
        self.project.figures.insert(fi + 1, copy);
        for (frame, pose) in self.project.frames.iter_mut().zip(frame_poses) {
            if let Some(p) = pose {
                frame.poses.insert(fi + 1, p);
            }
        }
        for (frame, bends) in self.project.frames.iter_mut().zip(frame_bends) {
            if let Some(b) = bends {
                frame.bends.insert(fi + 1, b);
            }
        }
        // Every stored figure index in the project remaps
        // (particles, FX-clip anchors, graph channels,
        // camera follow), so anchors keep pointing at the
        // same figure; the session's solo target shifts here
        // because it is editor state, not project data.
        remap_figures_after_insert(&mut self.project, fi + 1);
        if let Some(solo) = self.solo {
            if solo > fi {
                self.solo = Some(solo + 1);
            }
        }
        self.selected_figure = fi + 1;
        self.selected_node = None;
        self.status = "Figure duplicated.".into();
        Some(fi + 1)
    }

    /// Recolour figure `fi` as a whole: write `color` as
    /// every node's own colour — `NodeDef.color =
    /// Some(color)`, the same field `set_node_color`
    /// (editor.rs) writes one node at a time — under
    /// exactly ONE snapshot, so the recolour is a single
    /// undo step however many nodes the figure has.
    /// `set_node_color` itself never snapshots; this op
    /// snapshots once up front and then writes the field
    /// directly in the loop, so no per-node snapshots
    /// sneak in.
    ///
    /// Only the colour is written. A node's gradient end
    /// colour (`NodeStyle.color2`) is deliberately NOT
    /// recoloured: a gradient segment keeps its end
    /// colour and now runs from `color` to that end. The
    /// figure's own base colour (`FigureInstance.color`)
    /// is likewise left alone — with every node carrying
    /// an explicit colour it colours nothing until a
    /// node's colour is cleared back to `None`.
    ///
    /// Returns `false`, changing nothing but the status
    /// line (the refusal shape `duplicate_figure` uses),
    /// when `fi` names no figure or the figure has no
    /// nodes to recolour; a refusal takes no snapshot.
    pub fn recolor_figure(&mut self, fi: usize, color: u32) -> bool {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "No figure to recolour.".into();
            return false;
        };
        if fig.def.nodes.is_empty() {
            self.status = "Figure has no segments to recolour.".into();
            return false;
        }
        self.snapshot();
        for node in &mut self.project.figures[fi].def.nodes {
            node.color = Some(color);
        }
        self.status = format!("Figure {} recoloured.", fi + 1);
        true
    }

    /// Mirror figure `fi` on EVERY frame in the project,
    /// as one undo step. Each frame's pose for the figure
    /// is transformed with the same maths
    /// `mirror_selected_pose` (editor.rs) uses for a
    /// single frame — see [`mirror_pose_in_place`]: every
    /// angle negated, the root left in place. Exactly one
    /// snapshot is taken up front, so a single undo
    /// restores every frame's original pose.
    ///
    /// Frames whose pose table stores no row for the
    /// figure are skipped, matching `duplicate_figure`'s
    /// tolerance for ragged per-frame rows.
    ///
    /// Limit, shared with `mirror_selected_pose`: graph
    /// channels are NOT touched. An `Angle` channel
    /// overrides its joint's angle when a frame is
    /// evaluated, so a figure driven by graph channels
    /// will not visually mirror — the stored poses flip,
    /// the evaluated poses do not. Mirroring channel keys
    /// would be a graph op, not a figure op, and is
    /// deliberately out of scope here.
    ///
    /// Returns `false`, changing nothing but the status
    /// line (the refusal shape `recolor_figure` uses),
    /// when `fi` names no figure; a refusal takes no
    /// snapshot.
    pub fn mirror_figure_all_frames(&mut self, fi: usize) -> bool {
        if fi >= self.project.figures.len() {
            self.status = "No figure to mirror.".into();
            return false;
        }
        self.snapshot();
        let frame_count = self.project.frames.len();
        for frame in &mut self.project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                mirror_pose_in_place(pose);
            }
        }
        self.status = format!(
            "Figure {} mirrored on all {} frames.",
            fi + 1,
            frame_count
        );
        true
    }
}

// ---------------------------------------------------------------------
// MCP handlers (Wave 11, B1): the figure/pose surface the
// registry in mcp_tools.rs does not already cover — whole-
// figure recolour, rename, scale, centre/ground, cross-
// figure pose copy, pose reset, apply-pose-to-all-frames,
// the on-disk pose library, and the layers panel's bulk
// visibility ops. mcp_tools.rs's argument helpers are
// private to that file, so the small parsers below are
// deliberate local duplicates with `fop_` names (the same
// convention fx_ops/particle_ops follow). Every handler
// validates first and returns Err(String) on bad input —
// never panics — and mirrors the refusal wording of the
// panel/op it stands in for.
// ---------------------------------------------------------------------

fn fop_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn fop_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn fop_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match fop_get(args, key) {
        Some(v) => fop_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn fop_opt_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match fop_get(args, key) {
        Some(v) => fop_as_usize(v, key).map(Some),
        None => Ok(None),
    }
}

fn fop_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match fop_get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("argument '{key}' must be a number")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn fop_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match fop_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn fop_opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match fop_get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

fn fop_opt_bool(args: &Value, key: &str) -> Result<Option<bool>, String> {
    match fop_get(args, key) {
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Ok(None),
    }
}

fn fop_req_bool(args: &Value, key: &str) -> Result<bool, String> {
    fop_opt_bool(args, key)?
        .ok_or_else(|| format!("missing required argument '{key}'"))
}

/// Colours travel as `0xRRGGBB` integers (mcp_tools.rs's
/// `opt_color`, duplicated).
fn fop_req_color(args: &Value, key: &str) -> Result<u32, String> {
    match fop_get(args, key) {
        Some(v) => {
            let n = fop_as_usize(v, key)?;
            u32::try_from(n)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn fop_opt_root(args: &Value, key: &str) -> Result<Option<[f32; 2]>, String> {
    let Some(v) = fop_get(args, key) else {
        return Ok(None);
    };
    let arr = v
        .as_array()
        .ok_or_else(|| format!("argument '{key}' must be an [x, y] array"))?;
    if arr.len() != 2 {
        return Err(format!("argument '{key}' must have exactly 2 elements"));
    }
    let x = arr[0]
        .as_f64()
        .ok_or_else(|| format!("argument '{key}[0]' must be a number"))? as f32;
    let y = arr[1]
        .as_f64()
        .ok_or_else(|| format!("argument '{key}[1]' must be a number"))? as f32;
    Ok(Some([x, y]))
}

fn fop_check_figure(core: &EditorCore, fi: usize) -> Result<(), String> {
    if fi < core.project.figures.len() {
        Ok(())
    } else {
        Err(format!(
            "figure index {fi} out of range (project has {} figure(s))",
            core.project.figures.len()
        ))
    }
}

fn fop_check_frame(core: &EditorCore, frame: usize) -> Result<(), String> {
    if frame < core.project.frames.len() {
        Ok(())
    } else {
        Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ))
    }
}

/// The figure an optional `figure` argument names,
/// defaulting to the current selection (the built-in
/// delete/move figure tools' convention).
fn fop_target_figure(core: &EditorCore, args: &Value) -> Result<usize, String> {
    let fi = fop_opt_usize(args, "figure")?.unwrap_or(core.selected_figure);
    fop_check_figure(core, fi)?;
    Ok(fi)
}

fn fop_target_frame(core: &EditorCore, args: &Value) -> Result<usize, String> {
    let frame = fop_opt_usize(args, "frame")?.unwrap_or(core.current);
    fop_check_frame(core, frame)?;
    Ok(frame)
}

/// MCP `figure_recolor`: the figure inspector's Recolour —
/// [`EditorCore::recolor_figure`] (every node's own colour
/// under one snapshot). A refusal is the op's status line.
/// The reply's `color` is the stored override read back
/// from the model (the first node's — the op writes the
/// same value to every node), not the argument echoed:
/// the MCP-truth rule is that a reply states what the
/// model holds.
pub fn tool_figure_recolor(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_req_usize(args, "figure")?;
    fop_check_figure(core, fi)?;
    let color = fop_req_color(args, "color")?;
    if !core.recolor_figure(fi, color) {
        return Err(core.status.clone());
    }
    // A successful recolour means the figure has nodes
    // (the op refuses an empty figure), so node 0's stored
    // override exists and is every node's.
    let stored = core.project.figures[fi].def.nodes[0].color;
    Ok(json!({
        "ok": true,
        "figure": fi,
        "color": stored,
        "status": core.status,
    }))
}

/// MCP `figure_rename`: the figure inspector's Rename.
/// Since Roadmap Wave 1 (RM24, P-14 — A15 H3) an actual
/// name change takes exactly one undo step, here and in
/// the panel's `rename_selected_figure` alike (Wave 13
/// only marked it dirty); renaming to the name already
/// stored changes nothing and spends nothing. The name
/// is trimmed; blank is refused with the panel's wording.
pub fn tool_figure_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_req_usize(args, "figure")?;
    fop_check_figure(core, fi)?;
    let raw = fop_req_str(args, "name")?;
    let name = raw.trim();
    if name.is_empty() {
        return Err("Figure name cannot be empty.".into());
    }
    if core.project.figures[fi].name != name {
        core.snapshot_labeled("Rename figure");
        core.project.figures[fi].name = name.to_string();
    }
    core.status = format!("Figure {} renamed to \"{name}\".", fi + 1);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "name": core.project.figures[fi].name,
        "status": core.status,
    }))
}

/// MCP `figure_scale`: the figure inspector's scale
/// steppers, generalised to any positive factor. Mirrors
/// `scale_selected_figure` (inspector_figure.rs) exactly —
/// plan every parented node's post-scale length with
/// `adjust_length`'s arithmetic (clamped to [4.0, 220.0]),
/// refuse when nothing would move, otherwise one snapshot
/// and the writes through `adjust_length_raw` itself (the
/// raw write: since Roadmap Wave 1 (RM24, P-14) the
/// `adjust_length` door takes its own coalesced step, and
/// a composed scale must stay exactly one step). The op
/// acts on the SELECTED figure, so the target is selected
/// first — the built-in delete/move figure tools'
/// convention; the node selection is restored afterwards.
pub fn tool_figure_scale(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let factor = fop_req_f32(args, "factor")?;
    if !factor.is_finite() || factor <= 0.0 {
        return Err("Invalid scale factor.".into());
    }
    if !core.project.figures[fi]
        .def
        .nodes
        .iter()
        .any(|n| n.parent.is_some())
    {
        return Err(format!("Figure {} has no segments to scale.", fi + 1));
    }
    let nodes = &core.project.figures[fi].def.nodes;
    let mut before_total = 0.0f32;
    let mut after_total = 0.0f32;
    let mut any_change = false;
    for node in nodes {
        if node.parent.is_some() {
            let planned = (node.length * factor).clamp(4.0, 220.0);
            before_total += node.length;
            after_total += planned;
            if planned != node.length {
                any_change = true;
            }
        }
    }
    if !any_change {
        return Err(format!(
            "Figure {} is already at the segment size limit.",
            fi + 1
        ));
    }
    core.selected_figure = fi;
    core.snapshot_labeled("Scale figure");
    let saved_node = core.selected_node;
    let count = core.project.figures[fi].def.nodes.len();
    for ni in 0..count {
        let len = core.project.figures[fi].def.nodes[ni].length;
        core.selected_node = Some(ni);
        core.adjust_length_raw(len * factor - len);
    }
    core.selected_node = saved_node;
    // Report the achieved scale, as the panel does.
    if before_total > 0.0 && before_total.is_finite() && after_total.is_finite() {
        core.status = format!(
            "Figure {} scaled to {:.0}%.",
            fi + 1,
            after_total / before_total * 100.0
        );
    } else {
        core.status = format!(
            "Figure {} scaled toward {:.0}%; some segments stopped at the size limit.",
            fi + 1,
            factor * 100.0
        );
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "factor": factor,
        "status": core.status,
    }))
}

/// The drawn bounding box of figure `fi` at the display
/// frame — `inspector_figure::figure_bbox` recomputed
/// from lib primitives (`positions_f` + the project's
/// head-ring radius, head nodes expanded by the ring).
fn fop_figure_bbox(core: &EditorCore, fi: usize) -> Option<[f32; 4]> {
    let fig = core.project.figures.get(fi)?;
    let pos = core.positions_f(core.display_frame(), fi);
    if pos.is_empty() {
        return None;
    }
    let ring_r = core.project.head_ring_radius();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (i, p) in pos.iter().enumerate() {
        let r = if fig.def.nodes.get(i).map(|n| n.head).unwrap_or(false) {
            ring_r
        } else {
            0.0
        };
        min_x = min_x.min(p[0] - r);
        min_y = min_y.min(p[1] - r);
        max_x = max_x.max(p[0] + r);
        max_y = max_y.max(p[1] + r);
    }
    Some([min_x, min_y, max_x, max_y])
}

/// Shared tail of `figure_centre` / `figure_ground`: the
/// panel's validation (the current frame must hold a pose
/// row for the figure), then the root shift through
/// `nudge_path_point` — the core primitive the panel's
/// `shift_selected_figure_root` uses, which takes the one
/// snapshot.
fn fop_shift_figure_root(
    core: &mut EditorCore,
    fi: usize,
    delta: [f32; 2],
    status: String,
) -> Result<Value, String> {
    let frame = core.current;
    let valid = core
        .project
        .frames
        .get(frame)
        .map(|f| fi < f.poses.len())
        .unwrap_or(false);
    if !valid {
        return Err("No such figure.".into());
    }
    core.nudge_path_point(frame, fi, delta);
    core.status = status;
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "root": core.project.frames[frame].poses[fi].root,
        "status": core.status,
    }))
}

/// MCP `figure_centre`: the figure inspector's Centre
/// figure — shift the figure's root at the CURRENT frame
/// so its drawn bounding box centres on the stage. One
/// undo step (via `nudge_path_point`).
pub fn tool_figure_centre(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let Some(bbox) = fop_figure_bbox(core, fi) else {
        return Err("No such figure.".into());
    };
    let stage = core.project.stage;
    let delta = [
        stage[0] / 2.0 - (bbox[0] + bbox[2]) / 2.0,
        stage[1] / 2.0 - (bbox[1] + bbox[3]) / 2.0,
    ];
    fop_shift_figure_root(core, fi, delta, format!("Figure {} centred on the stage.", fi + 1))
}

/// MCP `figure_ground`: the figure inspector's Ground
/// figure — shift the figure's root at the CURRENT frame
/// so its drawn bounding box's bottom rests on the
/// stage's bottom edge. One undo step.
pub fn tool_figure_ground(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let Some(bbox) = fop_figure_bbox(core, fi) else {
        return Err("No such figure.".into());
    };
    let delta = [0.0, core.project.stage[1] - bbox[3]];
    fop_shift_figure_root(core, fi, delta, format!("Figure {} grounded.", fi + 1))
}

/// MCP `copy_pose_between`: the pose panel's Copy-pose-to
/// — [`EditorCore::copy_pose_between`] on the CURRENT
/// frame. The handler pre-checks with the panel's guards
/// (both pose rows exist; equal angle counts) so a refusal
/// is an Err, not a silent status; the core op re-checks
/// the same conditions itself.
pub fn tool_copy_pose_between(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let from = fop_req_usize(args, "from_figure")?;
    let to = fop_req_usize(args, "to_figure")?;
    fop_check_figure(core, from)?;
    fop_check_figure(core, to)?;
    if from == to {
        return Err("Copy pose failed: source and target are the same figure.".into());
    }
    let cur = core.current;
    let frame = core
        .project
        .frames
        .get(cur)
        .ok_or_else(|| "Copy pose failed: the project has no frames.".to_string())?;
    let src = frame.poses.get(from).ok_or_else(|| {
        format!("Figure {} has no pose on frame {}.", from + 1, cur + 1)
    })?;
    let tgt = frame.poses.get(to).ok_or_else(|| {
        format!("Figure {} has no pose on frame {}.", to + 1, cur + 1)
    })?;
    if src.angles.len() != tgt.angles.len() {
        return Err(format!(
            "Copy pose failed: figure {} has {} angles but figure {} has {} — poses only copy between figures with the same rig size.",
            from + 1,
            src.angles.len(),
            to + 1,
            tgt.angles.len()
        ));
    }
    core.copy_pose_between(from, to);
    Ok(json!({
        "ok": true,
        "from_figure": from,
        "to_figure": to,
        "frame": cur,
        "status": core.status,
    }))
}

/// MCP `reset_pose`: the pose panel's Reset pose — write
/// the figure's rest pose (root kept) over its pose in a
/// frame, one snapshot. `figure` defaults to the selected
/// figure, `frame` to the current frame.
pub fn tool_reset_pose(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let frame = fop_target_frame(core, args)?;
    let cur = core.project.frames[frame]
        .poses
        .get(fi)
        .ok_or_else(|| format!("Figure {} has no pose on frame {}.", fi + 1, frame + 1))?;
    let rest = core.project.figures[fi].def.rest_pose(cur.root);
    core.snapshot();
    core.project.frames[frame].poses[fi] = rest.clone();
    core.status = format!(
        "Figure {} reset to its rest pose on frame {}.",
        fi + 1,
        frame + 1
    );
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "pose": { "root": rest.root, "angles": rest.angles },
        "status": core.status,
    }))
}

/// MCP `set_rest_pose`: the figure inspector's
/// Set-rest-from-current-pose (Wave Y, Y7/Y8, P-10) —
/// the figure's rest (setup) angles become the angles
/// of the pose it shows at the current frame. `figure`
/// defaults to the selected figure; `rebase` (default
/// true) shifts every authored angle store by the
/// per-node rest delta so the animation keeps its
/// content against the new rig, while false changes
/// only the rest angles (absolute). The editor op owns
/// the snapshot and the status: a real change is one
/// undo step, and refusals and the already-rest no-op
/// spend none — both reported here as `changed: false`
/// with the op's status.
pub fn tool_set_rest_pose(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let rebase = fop_opt_bool(args, "rebase")?.unwrap_or(true);
    let changed = core.set_rest_from_pose(fi, rebase);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "rebase": rebase,
        "changed": changed,
        "rest_angles": core.project.figures[fi].def.rest_angles(),
        "status": core.status,
    }))
}

/// MCP `pose_apply_all_frames`: the pose panel's
/// Apply-pose-to-all-frames, with the pose supplied in the
/// call (MCP has no shell clipboard). One snapshot for the
/// whole batch; frames whose pose row is missing or whose
/// angle count differs from the supplied pose's are
/// skipped and counted, exactly as the panel's composed
/// op judges them. When `root` is omitted each frame
/// keeps its own root.
pub fn tool_pose_apply_all_frames(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_req_usize(args, "figure")?;
    fop_check_figure(core, fi)?;
    let angles_v = fop_get(args, "angles")
        .ok_or_else(|| "missing required argument 'angles'".to_string())?;
    let angles_arr = angles_v
        .as_array()
        .ok_or_else(|| "argument 'angles' must be an array of numbers".to_string())?;
    let mut angles: Vec<f32> = Vec::with_capacity(angles_arr.len());
    for (i, v) in angles_arr.iter().enumerate() {
        angles.push(
            v.as_f64()
                .map(|f| f as f32)
                .ok_or_else(|| format!("argument 'angles[{i}]' must be a number"))?,
        );
    }
    let root = fop_opt_root(args, "root")?;
    let mut writable: Vec<usize> = Vec::new();
    let mut skipped = 0usize;
    for (f, frame) in core.project.frames.iter().enumerate() {
        match frame.poses.get(fi) {
            Some(p) if p.angles.len() == angles.len() => writable.push(f),
            _ => skipped += 1,
        }
    }
    if writable.is_empty() {
        return Err(format!(
            "No frame could take the pose ({skipped} skipped)."
        ));
    }
    let applied = writable.len();
    core.snapshot();
    for f in writable {
        let r = root.unwrap_or(core.project.frames[f].poses[fi].root);
        core.project.frames[f].poses[fi] = Pose {
            root: r,
            angles: angles.clone(),
        };
    }
    core.status = if skipped == 0 {
        format!("Pose applied to {applied} frames (0 skipped).")
    } else {
        format!(
            "Pose applied to {applied} frames; {skipped} cannot take the copied pose (no pose row or a different rig)."
        )
    };
    Ok(json!({
        "ok": true,
        "figure": fi,
        "applied": applied,
        "skipped": skipped,
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// The on-disk pose library (./library/poses/<slug>.json,
// each file {"name": ..., "pose": {...}}). The shell's
// pose panel (rune_sec/pose_tools.rs) owns this format;
// its slug/entry code is bin-side, so the pieces below
// mirror it line for line — the same deliberate mirror
// pose_tools itself keeps of fig3d's library. If the
// format or slug rules change, both copies must change.
// ---------------------------------------------------------------------

fn fop_pose_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.extend(ch.to_lowercase());
        } else {
            dash = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("figure");
    }
    slug
}

fn fop_pose_file_name(name: &str) -> String {
    format!("{}.json", fop_pose_slug(name))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct FopPoseLibraryEntry {
    name: String,
    pose: Pose,
}

fn fop_pose_library_dir() -> PathBuf {
    // The pose folder inside the one resolver's library
    // dir (Roadmap Wave 1, RM9 / F-8).
    crate::shell::library_dir().join("poses")
}

fn fop_save_pose_library_in(dir: &Path, pose: &Pose, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(fop_pose_file_name(name));
    let entry = FopPoseLibraryEntry {
        name: name.to_string(),
        pose: pose.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The file backing the pose entry displayed as `name` in
/// directory `dir`: the file whose stored name IS `name`,
/// found by the same scan `fop_scan_pose_library_in` lists
/// from — the listing reads names out of file contents, so
/// resolution must read them too (a hand-placed or externally
/// renamed file whose stem is not its name's slug is listed,
/// and only this resolution finds it again). When no file
/// stores `name`, the historical slug path is returned, so
/// loading by a case/slug variant keeps working and honest
/// fs errors land on the canonical path. Duplicate stored
/// names resolve to the first file in sorted path order.
/// Mirrors `pose_entry_file_in` in rune_sec/pose_tools.rs —
/// the two copies change together (Wave W, W9).
fn fop_pose_entry_file_in(dir: &Path, name: &str) -> PathBuf {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let is_pose_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".json"))
                .unwrap_or(false);
            if is_pose_file {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<FopPoseLibraryEntry>(&text) {
                        if parsed.name == name {
                            return path;
                        }
                    }
                }
            }
        }
    }
    dir.join(fop_pose_file_name(name))
}

fn fop_load_pose_library_in(dir: &Path, name: &str) -> std::io::Result<Pose> {
    let path = fop_pose_entry_file_in(dir, name);
    let text = std::fs::read_to_string(&path)?;
    let entry: FopPoseLibraryEntry = serde_json::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(entry.pose)
}

fn fop_scan_pose_library_in(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_pose_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".json"))
                .unwrap_or(false);
            if is_pose_file {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<FopPoseLibraryEntry>(&text) {
                        names.push(parsed.name);
                    }
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// MCP `pose_library_list`: every pose saved in the 2D
/// pose library (the pose panel's library list), sorted.
pub fn tool_pose_library_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let _ = core;
    let names = fop_scan_pose_library_in(&fop_pose_library_dir());
    Ok(json!({ "ok": true, "poses": names }))
}

/// MCP `pose_library_save`: the pose panel's Save pose,
/// generalised — save a figure's pose from a frame under
/// `name` (default: the figure's display name, the
/// panel's own choice). A file write, NOT an undo step.
pub fn tool_pose_library_save(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let frame = fop_target_frame(core, args)?;
    let pose = core.project.frames[frame]
        .poses
        .get(fi)
        .cloned()
        .ok_or_else(|| format!("Figure {} has no pose on frame {}.", fi + 1, frame + 1))?;
    let name = match fop_opt_str(args, "name")? {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err("argument 'name' must not be empty".into());
            }
            trimmed.to_string()
        }
        None => {
            let fig_name = core.project.figures[fi].name.clone();
            if fig_name.trim().is_empty() {
                format!("Figure {}", fi + 1)
            } else {
                fig_name
            }
        }
    };
    let path = fop_save_pose_library_in(&fop_pose_library_dir(), &pose, &name)
        .map_err(|e| format!("Save pose failed: {e}"))?;
    core.status = format!("Saved pose '{name}'.");
    Ok(json!({
        "ok": true,
        "name": name,
        "path": path.display().to_string(),
        "figure": fi,
        "frame": frame,
        "status": core.status,
    }))
}

/// MCP `pose_library_apply`: the pose panel's library
/// Apply (and Apply-mirrored, via `mirror`) — load the
/// saved pose and write it over a figure's pose in a
/// frame, one snapshot, with Paste's guards (the target
/// pose row must exist; the angle count must match).
pub fn tool_pose_library_apply(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let name = fop_req_str(args, "name")?;
    let fi = fop_target_figure(core, args)?;
    let frame = fop_target_frame(core, args)?;
    let mirrored = fop_opt_bool(args, "mirror")?.unwrap_or(false);
    let cur_len = core.project.frames[frame]
        .poses
        .get(fi)
        .map(|p| p.angles.len())
        .ok_or_else(|| format!("Figure {} has no pose on frame {}.", fi + 1, frame + 1))?;
    let mut pose = fop_load_pose_library_in(&fop_pose_library_dir(), name)
        .map_err(|e| format!("Apply pose failed: '{name}' could not be loaded ({e})."))?;
    if pose.angles.len() != cur_len {
        return Err(format!(
            "Apply pose failed: saved pose has {} angles, figure {} has {} segments.",
            pose.angles.len(),
            fi + 1,
            cur_len
        ));
    }
    if mirrored {
        for a in &mut pose.angles {
            *a = -*a;
        }
    }
    core.snapshot();
    core.project.frames[frame].poses[fi] = pose.clone();
    core.status = if mirrored {
        format!(
            "Pose '{name}' applied mirrored to figure {} on frame {}.",
            fi + 1,
            frame + 1
        )
    } else {
        format!(
            "Pose '{name}' applied to figure {} on frame {}.",
            fi + 1,
            frame + 1
        )
    };
    Ok(json!({
        "ok": true,
        "name": name,
        "figure": fi,
        "frame": frame,
        "mirrored": mirrored,
        "pose": { "root": pose.root, "angles": pose.angles },
        "status": core.status,
    }))
}

/// MCP `pose_library_delete`: the pose panel's library
/// Delete. A file operation, NOT an undo step.
pub fn tool_pose_library_delete(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let name = fop_req_str(args, "name")?;
    let path = fop_pose_entry_file_in(&fop_pose_library_dir(), name);
    std::fs::remove_file(&path).map_err(|e| {
        format!("Delete pose failed: '{name}' could not be removed ({e}).")
    })?;
    core.status = format!("Pose '{name}' deleted from the library.");
    Ok(json!({
        "ok": true,
        "name": name,
        "deleted": true,
        "status": core.status,
    }))
}

/// MCP `pose_library_rename`: the pose panel's library
/// Rename — same plan (blank / same-name / slug-collision
/// rules) and same filesystem sequence (load, write the
/// new file, remove the old) as `rename_library_pose_in`.
/// A file operation, NOT an undo step.
pub fn tool_pose_library_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let old = fop_req_str(args, "name")?;
    let raw_new = fop_req_str(args, "new_name")?;
    let dir = fop_pose_library_dir();
    let names = fop_scan_pose_library_in(&dir);
    if !names.iter().any(|n| n.as_str() == old) {
        return Err(format!("no pose library entry named '{old}'"));
    }
    let new_name = raw_new.trim();
    if new_name.is_empty() {
        return Err("Type a new name for the pose first.".into());
    }
    if new_name == old {
        core.status = format!("Pose '{old}' already has that name.");
        return Ok(json!({
            "ok": true,
            "renamed": false,
            "name": old,
            "status": core.status,
        }));
    }
    let new_slug = fop_pose_slug(new_name);
    if let Some(conflict) = names.iter().find(|n| {
        n.as_str() != old && (n.as_str() == new_name || fop_pose_slug(n) == new_slug)
    }) {
        return Err(format!(
            "A pose named '{conflict}' already uses that name."
        ));
    }
    let old_path = fop_pose_entry_file_in(&dir, old);
    let new_path = dir.join(fop_pose_file_name(new_name));
    if new_path != old_path && new_path.exists() {
        return Err(format!(
            "A file named '{}' already exists in the pose library folder.",
            fop_pose_file_name(new_name)
        ));
    }
    let pose = fop_load_pose_library_in(&dir, old)
        .map_err(|e| format!("Rename pose failed: '{old}' could not be loaded ({e})."))?;
    fop_save_pose_library_in(&dir, &pose, new_name)
        .map_err(|e| format!("Rename pose failed: {e}"))?;
    if new_path != old_path {
        if let Err(e) = std::fs::remove_file(&old_path) {
            // The rename landed; the stale old file remains
            // and will list as a second entry. Say exactly
            // that, as the panel does.
            core.status = format!(
                "Pose '{old}' renamed to '{new_name}', but the old file cannot be removed ({e})."
            );
            return Ok(json!({
                "ok": true,
                "renamed": true,
                "name": new_name,
                "warning": core.status,
                "status": core.status,
            }));
        }
    }
    core.status = format!("Pose '{old}' renamed to '{new_name}'.");
    Ok(json!({
        "ok": true,
        "renamed": true,
        "name": new_name,
        "status": core.status,
    }))
}

/// MCP `layers_set_all_visible`: the figures/layers
/// panels' All / None buttons — `layers::set_all_visible`.
/// Returns how many figures changed state. Since Roadmap
/// Wave 1 (RM24, P-14 — A15 H3) the bulk write takes
/// exactly one undo step when at least one flag actually
/// changes (the would-change count is computed before
/// the write, so the step precedes it; a no-change call
/// spends nothing). Wave 13 only marked it dirty.
pub fn tool_layers_set_all_visible(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let visible = fop_req_bool(args, "visible")?;
    let would_change = core
        .project
        .figures
        .iter()
        .filter(|f| f.hidden == visible)
        .count();
    if would_change > 0 {
        core.snapshot_labeled("Figure visibility");
    }
    let changed = crate::layers::set_all_visible(&mut core.project, visible);
    // Panel parity (figures_panel footer, reconciled at Wave 11
    // integration): the bulk write also clears any active solo
    // so the flags ARE the effective visibility, and reports
    // the count in the status line, in the panel's voice.
    core.solo = None;
    core.status = if visible {
        format!("All figures shown ({changed} changed).")
    } else {
        format!("All figures hidden ({changed} changed).")
    };
    Ok(json!({
        "ok": true,
        "visible": visible,
        "changed": changed,
        "figures": core.project.figures.len(),
        "status": core.status,
    }))
}

/// MCP `layers_invert_visibility`: the figures panel's
/// Invert button — `layers::invert_visibility`. Returns
/// how many figures changed state. Since Roadmap Wave 1
/// (RM24, P-14): as in `layers_set_all_visible`, exactly
/// one undo step when at least one flag flips (with at
/// least one figure present an invert always flips, so
/// the step is taken whenever the project is non-empty).
pub fn tool_layers_invert_visibility(
    core: &mut EditorCore,
    _args: &Value,
) -> Result<Value, String> {
    if !core.project.figures.is_empty() {
        core.snapshot_labeled("Figure visibility");
    }
    let changed = crate::layers::invert_visibility(&mut core.project);
    // Panel parity, as in tool_layers_set_all_visible.
    core.solo = None;
    core.status = format!("Figure visibility inverted ({changed} flipped).");
    Ok(json!({
        "ok": true,
        "changed": changed,
        "figures": core.project.figures.len(),
        "status": core.status,
    }))
}

/// MCP handler: cycle one figure's opacity through the
/// panel's steps (the figure inspector's Opacity cycler —
/// `EditorCore::cycle_figure_opacity`). The step is taken
/// inside the core op (coalesced since Roadmap Wave 1,
/// RM24, P-14; before, this cycler took no snapshot at
/// all). Wave 11 (C1), integrator-authored.
pub fn tool_figure_cycle_opacity(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_opt_usize(args, "figure")?.unwrap_or(core.selected_figure);
    if fi >= core.project.figures.len() {
        return Err(format!("no figure at index {fi}"));
    }
    core.cycle_figure_opacity(fi);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "opacity": core.project.figures[fi].opacity,
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Wave 14 MCP handlers (A2): the figure panel's relative
// and toggle controls that had no dedicated tool — the
// opacity −/+ steppers, the hidden / lock / solo toggle
// buttons (the figures panel rows and the palette's
// hide-figure / lock-figure / solo-figure commands), and
// the motion-path nudge primitive the figure inspector's
// Centre/Ground compose from. The `layers` tool can SET
// hidden / locked / opacity / solo absolutely, and
// `set_pose` can set a root absolutely, but until now
// each of these panel actions took a read-then-write
// pair of calls to reproduce. Every handler validates
// all of its arguments (indices quoted, floats finite)
// BEFORE the op runs and never panics. The TOOL ENTRY
// SPEC above each handler is the registry entry + MCP.md
// row the integrator splices verbatim.
//
// Undo discipline, stated plainly because it is the
// documented exception family: the layers-family ops
// below (step / toggles) take NO snapshot in the core —
// panel parity, the same rule the `layers` tool and
// `figure_cycle_opacity` already document. A real
// project change counts once for dirty/autosave via
// `mark_dirty` inside the ops; solo is session state
// and counts nothing. `nudge_path_point` is the
// exception's opposite: the op snapshots exactly once.
// ---------------------------------------------------------------------

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside
// figure_cycle_opacity.
//   Tool {
//       name: "figure_step_opacity",
//       description: "Step a figure's opacity by 'delta' (the figures/layers panels' opacity −/+ buttons): the target is rounded to whole percent and clamped to 0.05-1.0 by the op, and the reply reports the opacity read back after clamping. The relative form of the layers tool's opacity. Not an undo step (layers-family panel parity); a real change counts for dirty/autosave.",
//       schema: obj_schema(
//           json!({
//               "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" },
//               "delta": { "type": "number", "description": "Opacity step, e.g. 0.1 or -0.1 (non-zero, finite)" }
//           }),
//           &["delta"],
//       ),
//       handler: crate::figure_ops::tool_figure_step_opacity,
//   }
// MCP.md row:
//   | `figure_step_opacity` | Step a figure's opacity by 'delta' (the figures/layers panels' −/+ buttons): the target is rounded to whole percent and clamped to 0.05–1.0 by the op; the reply reports the stored opacity read back, so a step past a bound truthfully reports the bound. The relative form of the `layers` tool's opacity. Not an undo step (layers-family panel parity); a real change counts for dirty/autosave. | `delta` | `figure` |
/// MCP `figure_step_opacity`: the figures/layers panels'
/// opacity −/+ steppers — [`EditorCore::step_figure_opacity`].
pub fn tool_figure_step_opacity(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let delta = fop_req_f32(args, "delta")?;
    if !delta.is_finite() {
        return Err("argument 'delta' must be a finite number".into());
    }
    if delta == 0.0 {
        return Err("argument 'delta' must be non-zero".into());
    }
    core.step_figure_opacity(fi, delta);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "opacity": core.project.figures[fi].opacity,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside
// figure_cycle_opacity.
//   Tool {
//       name: "figure_toggle_hidden",
//       description: "Toggle a figure's hidden flag (the figures panel's hide button / the palette's Toggle figure hidden): the relative form of the layers tool's 'hidden'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave.",
//       schema: obj_schema(
//           json!({
//               "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
//           }),
//           &[],
//       ),
//       handler: crate::figure_ops::tool_figure_toggle_hidden,
//   }
// MCP.md row:
//   | `figure_toggle_hidden` | Toggle a figure's hidden flag (the figures panel's hide button; the palette's Toggle figure hidden) — the relative form of the `layers` tool's 'hidden'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
/// MCP `figure_toggle_hidden`: the figures panel's hide
/// button — [`EditorCore::toggle_figure_hidden`], which
/// acts on the SELECTED figure, so the target is
/// selected first (the built-in delete/move figure
/// tools' convention).
pub fn tool_figure_toggle_hidden(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    core.selected_figure = fi;
    core.toggle_figure_hidden();
    Ok(json!({
        "ok": true,
        "figure": fi,
        "hidden": core.project.figures[fi].hidden,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside
// figure_cycle_opacity.
//   Tool {
//       name: "figure_toggle_lock",
//       description: "Toggle a figure's locked flag (the figures panel's lock button / the palette's Lock selected figure): the relative form of the layers tool's 'locked'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave.",
//       schema: obj_schema(
//           json!({
//               "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
//           }),
//           &[],
//       ),
//       handler: crate::figure_ops::tool_figure_toggle_lock,
//   }
// MCP.md row:
//   | `figure_toggle_lock` | Toggle a figure's locked flag (the figures panel's lock button; the palette's Lock selected figure) — the relative form of the `layers` tool's 'locked'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
/// MCP `figure_toggle_lock`: the figures panel's lock
/// button — [`EditorCore::toggle_figure_lock`].
pub fn tool_figure_toggle_lock(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    core.toggle_figure_lock(fi);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "locked": core.project.figures[fi].locked,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside
// figure_cycle_opacity.
//   Tool {
//       name: "figure_toggle_solo",
//       description: "Toggle a figure's solo state (the figures panel's solo button / the palette's Solo selected figure): soloing a figure shows it alone; toggling the soloed figure clears solo. The relative form of the layers tool's 'solo'. Session state only — not an undo step and not a project edit.",
//       schema: obj_schema(
//           json!({
//               "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
//           }),
//           &[],
//       ),
//       handler: crate::figure_ops::tool_figure_toggle_solo,
//   }
// MCP.md row:
//   | `figure_toggle_solo` | Toggle a figure's solo state (the figures panel's solo button; the palette's Solo selected figure) — the relative form of the `layers` tool's 'solo'. Session state only: not an undo step and not a project edit. | — | `figure` |
/// MCP `figure_toggle_solo`: the figures panel's solo
/// button — [`EditorCore::toggle_solo`].
pub fn tool_figure_toggle_solo(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    core.toggle_solo(fi);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "solo": core.solo == Some(fi),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside figure_ground.
//   Tool {
//       name: "nudge_path_point",
//       description: "Nudge one figure's motion-path point: move the root of its pose at 'frame' by ('dx', 'dy') stage units (the stage's path-point drag, as data; figure_centre/figure_ground compose this primitive for their specific shifts). 'figure' defaults to the selected figure, 'frame' to the current frame. One undo step.",
//       schema: obj_schema(
//           json!({
//               "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" },
//               "frame": { "type": "integer", "description": "0-based frame index (default: the current frame)" },
//               "dx": { "type": "number", "description": "Horizontal shift in stage units (finite)" },
//               "dy": { "type": "number", "description": "Vertical shift in stage units (finite)" }
//           }),
//           &["dx", "dy"],
//       ),
//       handler: crate::figure_ops::tool_nudge_path_point,
//   }
// MCP.md row:
//   | `nudge_path_point` | Nudge one figure's motion-path point: move the root of its pose at 'frame' by ('dx', 'dy') stage units — the stage's path-point drag as data; `figure_centre`/`figure_ground` compose this same primitive for their specific shifts. 'figure' defaults to the selected figure, 'frame' to the current frame. One undo step. | `dx`, `dy` | `figure`, `frame` |
/// MCP `nudge_path_point`: the motion-path primitive —
/// [`EditorCore::nudge_path_point`]. The handler
/// pre-checks the pose row exists, so the op's own
/// status-only refusal never spends its snapshot on a
/// target that cannot move.
pub fn tool_nudge_path_point(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_target_figure(core, args)?;
    let frame = fop_target_frame(core, args)?;
    let dx = fop_req_f32(args, "dx")?;
    let dy = fop_req_f32(args, "dy")?;
    if !dx.is_finite() || !dy.is_finite() {
        return Err("arguments 'dx' and 'dy' must be finite numbers".into());
    }
    if dx == 0.0 && dy == 0.0 {
        return Err("arguments 'dx' and 'dy' must not both be zero".into());
    }
    let has_pose = core
        .project
        .frames
        .get(frame)
        .map(|f| fi < f.poses.len())
        .unwrap_or(false);
    if !has_pose {
        return Err(format!(
            "Figure {} has no pose on frame {}.",
            fi + 1,
            frame + 1
        ));
    }
    core.nudge_path_point(frame, fi, [dx, dy]);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "root": core.project.frames[frame].poses[fi].root,
        "status": core.status,
    }))
}

/// MCP handler: read one figure in full (the figures panel
/// row + figure inspector as data): identity, appearance
/// flags, node count, clip link, solo state, and the drawn
/// bounding box on the current frame when the figure has a
/// pose there. Read-only. Wave 11 (C10 G3), integrator-
/// authored.
pub fn tool_get_figure(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_req_usize(args, "figure")?;
    let Some(f) = core.project.figures.get(fi) else {
        return Err(format!("no figure at index {fi}"));
    };
    let bbox = fop_figure_bbox(core, fi);
    Ok(json!({
        "ok": true,
        "index": fi,
        "name": f.name,
        "color": f.color,
        "hidden": f.hidden,
        "locked": f.locked,
        "opacity": f.opacity,
        "nodes": f.def.nodes.len(),
        "clip": f.clip,
        "solo": core.solo == Some(fi),
        "bbox": bbox.map(|b| json!({
            "min_x": b[0], "min_y": b[1], "max_x": b[2], "max_y": b[3],
        })),
    }))
}

/// MCP handler: read one node in full — the read keystone
/// the surface lacked (C10 G4): the definition (name, parent,
/// length, thickness, colour override, shape, head flag), the
/// segment style (via `get_node_style`'s own read), and the
/// pose facts at a frame (default: current): the stored
/// parent-relative angle, the solved world direction of the
/// bone ending at this joint in degrees [0, 360), and the
/// frame's bend in radians and degrees. Read-only.
/// Integrator-authored.
pub fn tool_get_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fop_req_usize(args, "figure")?;
    let ni = fop_req_usize(args, "node")?;
    let frame = fop_opt_usize(args, "frame")?.unwrap_or(core.current);
    let Some(f) = core.project.figures.get(fi) else {
        return Err(format!("no figure at index {fi}"));
    };
    // Owned copy: the style read below takes `core` mutably,
    // so the reply's node facts are extracted up front.
    let Some(node) = f.def.nodes.get(ni).cloned() else {
        return Err(format!("figure {fi} has no node at index {ni}"));
    };
    let node = &node;
    if frame >= core.project.frames.len() {
        return Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ));
    }
    let style = crate::style::tool_get_node_style(
        core,
        &json!({ "figure": fi, "node": ni }),
    )?;
    let pose_angle = core
        .project
        .frames
        .get(frame)
        .and_then(|fr| fr.poses.get(fi))
        .and_then(|p| p.angles.get(ni).copied());
    let world_angle = node.parent.and_then(|p| {
        let positions = core.positions(frame, fi);
        let a = positions.get(p)?;
        let b = positions.get(ni)?;
        let deg = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
        Some(deg.rem_euclid(360.0))
    });
    let bend = core
        .project
        .frames
        .get(frame)
        .and_then(|fr| fr.bends.get(fi))
        .and_then(|row| row.get(ni).copied())
        .unwrap_or(0.0);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "name": node.name,
        "parent": node.parent,
        "length": node.length,
        "thickness": node.thickness,
        "color": node.color,
        "shape": match node.shape {
            crate::model::Shape::Line => "line".to_string(),
            crate::model::Shape::Circle => "circle".to_string(),
            crate::model::Shape::Polygon(n) => format!("polygon:{n}"),
        },
        "head": node.head,
        "style": {
            "color": style["color"],
            "effective_color": style["effective_color"],
            "color2": style["color2"],
            "cap": style["cap"],
            "taper": style["taper"],
            "opacity": style["opacity"],
        },
        "frame": frame,
        "pose_angle": pose_angle,
        "world_angle": world_angle,
        "bend_radians": bend,
        "bend_degrees": bend.to_degrees(),
        // The segment's attached media (Wave 12, B10 splice):
        // null when the segment carries none, otherwise an
        // object — {kind: "image", asset, offset, scale,
        // angle_offset, flip} naming the asset table index
        // and the sprite's local transform (Wave Y, P-8), or
        // {kind: "text", content, size}. The construction is
        // media::segment_media_json, shared with the
        // clip-side read so the two can never drift.
        // Additive: readers written before this field existed
        // simply never look at it.
        "media": crate::media::segment_media_json(&node.media),
    }))
}

/// MCP handler: list the 2D FIGURE library (the library
/// panel's entries): the FigureDefs saved in ./library, by
/// index and name, with the session's current library index.
/// Note the format truth: the library stores FigureDef only —
/// an entry's colour is NOT stored (insert assigns a palette
/// colour). An entry's name is the figure name stored
/// inside its file at save time (Wave V, V1); files saved
/// before the stored name existed are named by their file
/// stem. Read-only. Wave 11 (B1/D8), integrator-authored.
pub fn tool_figure_library_list(
    core: &mut EditorCore,
    _args: &Value,
) -> Result<Value, String> {
    let entries: Vec<Value> = core
        .library
        .iter()
        .enumerate()
        .map(|(i, (name, def))| {
            json!({
                "index": i,
                "name": name,
                "nodes": def.nodes.len(),
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "count": entries.len(),
        "library_index": core.library_index,
        "entries": entries,
    }))
}

/// MCP handler: save a figure's definition to the 2D figure
/// library (the library panel's Save — `EditorCore::
/// save_figure_to_library`, which writes the SELECTED
/// figure; 'figure' selects it first). The saved entry is
/// named after the figure. A file write, not an undo step.
/// Integrator-authored.
pub fn tool_figure_library_save(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    if let Some(fi) = fop_opt_usize(args, "figure")? {
        if fi >= core.project.figures.len() {
            return Err(format!("no figure at index {fi}"));
        }
        core.selected_figure = fi;
    }
    if core.selected_figure >= core.project.figures.len() {
        return Err("no figure selected to save".to_string());
    }
    core.save_figure_to_library();
    if core.status.contains("failed") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": core.selected_figure,
        "entries": core.library.len(),
        "status": core.status,
    }))
}

/// MCP handler: insert a figure from the 2D figure library
/// (the library panel's Insert — `EditorCore::
/// insert_library_figure`, which inserts the entry at the
/// session's library index; 'index' or 'name' chooses the
/// entry first). The inserted figure gets a palette colour
/// (the library stores definitions, not colours) and becomes
/// the selection. One undo step (inside the op).
/// Integrator-authored.
pub fn tool_figure_library_insert(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    if core.library.is_empty() {
        return Err("figure library is empty — save a figure first".to_string());
    }
    if let Some(name) = fop_opt_str(args, "name")? {
        let Some(pos) = core.library.iter().position(|(n, _)| n == name) else {
            return Err(format!("no library entry named '{name}'"));
        };
        core.library_index = pos;
    } else if let Some(idx) = fop_opt_usize(args, "index")? {
        if idx >= core.library.len() {
            return Err(format!("no library entry at index {idx}"));
        }
        core.library_index = idx;
    }
    let before = core.project.figures.len();
    core.insert_library_figure();
    if core.project.figures.len() != before + 1 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": before,
        "library_index": core.library_index,
        "name": core.project.figures[before].name,
        "status": core.status,
    }))
}

/// MCP handler: cycle the 2D figure library selection to the
/// next entry (the library panel's cycle — `EditorCore::
/// cycle_library`). Selection only; inserts nothing.
/// Integrator-authored.
pub fn tool_figure_library_cycle(
    core: &mut EditorCore,
    _args: &Value,
) -> Result<Value, String> {
    if core.library.is_empty() {
        return Err("figure library is empty — save a figure first".to_string());
    }
    core.cycle_library();
    Ok(json!({
        "ok": true,
        "library_index": core.library_index,
        "name": core.library[core.library_index].0,
        "status": core.status,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParticleEvent, ParticleKind};

    #[test]
    fn mcp_wave14_step_opacity_and_toggles_follow_the_layers_family_contract() {
        // Wave 14 (A2): the relative/toggle handlers'
        // bookkeeping. Roadmap Wave 1 (RM24, P-14) closed
        // the census this test originally pinned: the
        // layers family now snapshots — a real change
        // counts exactly one edit and one (coalesced) undo
        // step — and solo, session state, still counts
        // nothing.
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();
        assert_eq!(core.project.figures[0].opacity, 1.0);

        let out = tool_figure_step_opacity(&mut core, &json!({ "figure": 0, "delta": -0.1 }))
            .expect("step succeeds");
        // Opacity is stored f32: 1.0f32 - 0.1f32 widens to
        // 0.899999976… — compare within f32 tolerance.
        let opacity = out["opacity"].as_f64().unwrap();
        assert!((opacity - 0.9).abs() < 1e-6, "opacity steps to 0.9, got {opacity}");
        assert_eq!(core.undo_depth(), depth + 1, "step takes one coalesced undo step");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "the real change counts one edit"
        );

        // A step at the clamp changes nothing and spends
        // nothing (the op's own rule).
        let out = tool_figure_step_opacity(&mut core, &json!({ "figure": 0, "delta": 5.0 }))
            .expect("step to the ceiling succeeds");
        assert_eq!(out["opacity"], json!(1.0));
        let edits_at_ceiling = core.edits_since_autosave();
        let depth_at_ceiling = core.undo_depth();
        let _ = tool_figure_step_opacity(&mut core, &json!({ "figure": 0, "delta": 0.1 }))
            .expect("step at the ceiling still answers");
        assert_eq!(
            core.edits_since_autosave(),
            edits_at_ceiling,
            "a no-change step counts nothing"
        );
        assert_eq!(
            core.undo_depth(),
            depth_at_ceiling,
            "a no-change step spends no undo step"
        );

        let out = tool_figure_toggle_hidden(&mut core, &json!({ "figure": 0 }))
            .expect("hide succeeds");
        assert_eq!(out["hidden"], json!(true));
        assert!(core.project.figures[0].hidden);
        let out = tool_figure_toggle_hidden(&mut core, &json!({}))
            .expect("hide toggles back, defaulting to the selection");
        assert_eq!(out["hidden"], json!(false));

        let out = tool_figure_toggle_lock(&mut core, &json!({ "figure": 0 }))
            .expect("lock succeeds");
        assert_eq!(out["locked"], json!(true));

        let edits_before_solo = core.edits_since_autosave();
        let out = tool_figure_toggle_solo(&mut core, &json!({ "figure": 0 }))
            .expect("solo succeeds");
        assert_eq!(out["solo"], json!(true));
        assert_eq!(core.solo, Some(0));
        assert_eq!(
            core.edits_since_autosave(),
            edits_before_solo,
            "solo is session state, not a project edit"
        );
        let out = tool_figure_toggle_solo(&mut core, &json!({ "figure": 0 }))
            .expect("solo toggles off");
        assert_eq!(out["solo"], json!(false));
        assert_eq!(core.solo, None);
    }

    #[test]
    fn mcp_wave14_figure_handlers_refuse_bad_input_without_side_effects() {
        // Wave 14 (A2): every refusal is validated BEFORE
        // the op runs — bad indices quote the index, a
        // zero or non-finite delta is an argument error,
        // and nothing mutates.
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();

        let err = tool_figure_step_opacity(&mut core, &json!({ "figure": 9, "delta": 0.1 }))
            .expect_err("a missing figure is refused");
        assert!(err.contains("figure index 9 out of range"), "err: {err}");
        let err = tool_figure_step_opacity(&mut core, &json!({ "figure": 0, "delta": 0.0 }))
            .expect_err("a zero step is refused");
        assert!(err.contains("non-zero"), "err: {err}");
        let err = tool_figure_step_opacity(&mut core, &json!({ "figure": 0, "delta": 1e300 }))
            .expect_err("a delta that overflows the f32 cast is refused");
        assert!(err.contains("finite"), "err: {err}");
        let err = tool_figure_step_opacity(&mut core, &json!({ "figure": 0 }))
            .expect_err("delta is required");
        assert!(err.contains("missing required argument 'delta'"), "err: {err}");

        for handler in [
            tool_figure_toggle_hidden as fn(&mut EditorCore, &Value) -> Result<Value, String>,
            tool_figure_toggle_lock,
            tool_figure_toggle_solo,
        ] {
            let err = handler(&mut core, &json!({ "figure": 9 }))
                .expect_err("a missing figure is refused");
            assert!(err.contains("figure index 9 out of range"), "err: {err}");
        }

        assert_eq!(core.edits_since_autosave(), edits, "no refusal marked dirty");
        assert_eq!(core.undo_depth(), depth, "no refusal took a snapshot");
        assert!(!core.project.figures[0].hidden);
        assert!(!core.project.figures[0].locked);
        assert_eq!(core.solo, None);
    }

    #[test]
    fn mcp_wave14_nudge_path_point_moves_one_root_in_one_undo_step() {
        // Wave 14 (A2): the nudge spends exactly the op's
        // one snapshot, moves only the target pose's root,
        // and every refusal (bad index, zero vector,
        // non-finite component, missing pose row) is
        // validated before the op and spends nothing.
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        let root0 = core.project.frames[0].poses[0].root;

        let out = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 0, "frame": 0, "dx": 5.0, "dy": -3.0 }),
        )
        .expect("nudge succeeds");
        assert_eq!(out["root"], json!([root0[0] + 5.0, root0[1] - 3.0]));
        assert_eq!(core.undo_depth(), depth + 1, "exactly one undo step");
        core.undo();
        assert_eq!(core.project.frames[0].poses[0].root, root0, "one undo restores the root");

        let err = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 9, "frame": 0, "dx": 1.0, "dy": 1.0 }),
        )
        .expect_err("a missing figure is refused");
        assert!(err.contains("figure index 9 out of range"), "err: {err}");
        let err = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 0, "frame": 99, "dx": 1.0, "dy": 1.0 }),
        )
        .expect_err("a missing frame is refused");
        assert!(err.contains("frame index 99 out of range"), "err: {err}");
        let err = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 0, "frame": 0, "dx": 0.0, "dy": 0.0 }),
        )
        .expect_err("a zero nudge is refused");
        assert!(err.contains("must not both be zero"), "err: {err}");
        let err = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 0, "frame": 0, "dx": 1e300, "dy": 1.0 }),
        )
        .expect_err("a component that overflows the f32 cast is refused");
        assert!(err.contains("finite"), "err: {err}");

        // A frame whose pose row lacks the figure: refused
        // by the handler's pre-check, no snapshot spent.
        core.project.frames[0].poses.clear();
        let err = tool_nudge_path_point(
            &mut core,
            &json!({ "figure": 0, "frame": 0, "dx": 1.0, "dy": 1.0 }),
        )
        .expect_err("a missing pose row is refused");
        assert!(err.contains("has no pose on frame 1"), "err: {err}");
        assert_eq!(core.undo_depth(), depth, "no refusal took a snapshot");
    }

    #[test]
    fn mcp_figure_recolor_and_rename_validate_and_write() {
        let mut core = EditorCore::new();
        let out = tool_figure_recolor(&mut core, &json!({ "figure": 0, "color": 0x224466 }))
            .expect("recolor succeeds");
        assert_eq!(out["color"], json!(0x224466));
        assert!(core.project.figures[0]
            .def
            .nodes
            .iter()
            .all(|n| n.color == Some(0x224466)));
        assert!(tool_figure_recolor(&mut core, &json!({ "figure": 9, "color": 1 })).is_err());
        assert!(tool_figure_recolor(&mut core, &json!({ "figure": 0 })).is_err());

        let out = tool_figure_rename(&mut core, &json!({ "figure": 0, "name": "  Hero  " }))
            .expect("rename succeeds");
        assert_eq!(out["name"], json!("Hero"));
        assert_eq!(core.project.figures[0].name, "Hero");
        let err = tool_figure_rename(&mut core, &json!({ "figure": 0, "name": "   " }))
            .expect_err("blank names are refused");
        assert!(err.contains("cannot be empty"), "{err}");
    }

    #[test]
    fn mcp_figure_recolor_and_rename_dirty_and_undo_contract() {
        // Wave 13 (B3, the E8 census fix): the two MCP
        // paths' bookkeeping, pinned separately because
        // their contracts differed. Recolour is an undoable
        // edit — `recolor_figure` takes exactly one
        // snapshot, so the autosave counter and the undo
        // depth both advance by one. Rename was a plain
        // field write that stayed non-undoable (counter via
        // `mark_dirty`, depth unmoved) — Roadmap Wave 1
        // (RM24, P-14) closed that: rename now takes
        // exactly one undo step of its own.
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();

        tool_figure_recolor(&mut core, &json!({ "figure": 0, "color": 0x224466 }))
            .expect("recolor succeeds");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "recolour counts exactly one edit"
        );
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "recolour is exactly one undo step"
        );

        tool_figure_rename(&mut core, &json!({ "figure": 0, "name": "Hero" }))
            .expect("rename succeeds");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 2,
            "rename counts exactly one more edit"
        );
        assert_eq!(
            core.undo_depth(),
            depth + 2,
            "rename takes its own undo step since RM24"
        );
        assert_eq!(core.undo_label(), Some("Rename figure"));

        // Refusals spend nothing under either contract.
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();
        assert!(tool_figure_recolor(&mut core, &json!({ "figure": 9, "color": 1 })).is_err());
        assert!(tool_figure_rename(&mut core, &json!({ "figure": 0, "name": "   " })).is_err());
        assert!(tool_figure_rename(&mut core, &json!({ "figure": 9, "name": "X" })).is_err());
        assert_eq!(core.edits_since_autosave(), edits, "refusals mark no edit");
        assert_eq!(core.undo_depth(), depth, "refusals take no snapshot");
    }

    #[test]
    fn mcp_figure_recolor_reports_stored_colour_and_leaves_the_style_fields() {
        let mut core = EditorCore::new();
        // Give node 1 a non-default style. A recolour writes
        // node colours only, so every NodeStyle field must
        // survive untouched, and the reply must state the
        // stored override (read back), per the MCP-truth rule.
        {
            let node = &mut core.project.figures[0].def.nodes[1];
            node.style.taper = 0.45;
            node.style.cap = crate::model::CapKind::Point;
            node.style.color2 = Some(0x112233);
            node.style.opacity = 0.5;
        }
        let out = tool_figure_recolor(&mut core, &json!({ "figure": 0, "color": 0x224466 }))
            .expect("recolor succeeds");
        let stored = core.project.figures[0].def.nodes[1].color;
        assert_eq!(stored, Some(0x224466));
        assert_eq!(out["color"], json!(stored), "the reply reads the model back");
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.style.taper, 0.45);
        assert_eq!(node.style.cap, crate::model::CapKind::Point);
        assert_eq!(node.style.color2, Some(0x112233));
        assert_eq!(node.style.opacity, 0.5);
    }

    #[test]
    fn mcp_get_node_style_block_is_the_style_read_verbatim() {
        let mut core = EditorCore::new();
        {
            let node = &mut core.project.figures[0].def.nodes[1];
            node.color = Some(0x334455);
            node.style.taper = 0.45;
            node.style.cap = crate::model::CapKind::Point;
            node.style.color2 = Some(0x667788);
            node.style.opacity = 0.5;
        }
        let full = tool_get_node(&mut core, &json!({ "figure": 0, "node": 1 }))
            .expect("get_node succeeds");
        let direct = crate::style::tool_get_node_style(
            &mut core,
            &json!({ "figure": 0, "node": 1 }),
        )
        .expect("get_node_style succeeds");
        // No normalisation in the passthrough: every style
        // field get_node reports is the style read's own
        // value, and both are the stored values.
        for key in ["color", "effective_color", "color2", "cap", "taper", "opacity"] {
            assert_eq!(full["style"][key], direct[key], "field {key}");
        }
        assert!(
            (full["style"]["taper"].as_f64().unwrap() - 0.45).abs() < 1e-6,
            "stored taper reads back unclamped and unrounded: {full}"
        );
        assert_eq!(full["style"]["cap"], json!("point"));
        assert_eq!(full["style"]["opacity"], json!(0.5));
        assert_eq!(full["style"]["color2"], json!(0x667788));
    }

    #[test]
    fn mcp_figure_cycle_opacity_reports_the_stored_value() {
        let mut core = EditorCore::new();
        let out = tool_figure_cycle_opacity(&mut core, &json!({ "figure": 0 }))
            .expect("cycle succeeds");
        assert_eq!(core.project.figures[0].opacity, 0.6, "1.0 cycles to 0.6");
        assert_eq!(
            out["opacity"],
            json!(core.project.figures[0].opacity),
            "the reply is the stored value"
        );
        // From an off-ladder value the cycle snaps down
        // (layers::cycle_opacity) and the reply still states
        // the stored value, not a ladder assumption.
        core.project.figures[0].opacity = 0.5;
        let out = tool_figure_cycle_opacity(&mut core, &json!({ "figure": 0 }))
            .expect("cycle succeeds");
        assert_eq!(core.project.figures[0].opacity, 0.3);
        assert_eq!(out["opacity"], json!(core.project.figures[0].opacity));
        assert!(tool_figure_cycle_opacity(&mut core, &json!({ "figure": 9 })).is_err());
    }

    #[test]
    fn duplicate_figure_preserves_node_styles_verbatim() {
        let mut core = EditorCore::new();
        {
            let node = &mut core.project.figures[0].def.nodes[1];
            node.style.taper = 0.45;
            node.style.cap = crate::model::CapKind::Point;
            node.style.color2 = Some(0x112233);
            node.style.opacity = 0.5;
        }
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(
            core.project.figures[1].def.nodes[1].style,
            core.project.figures[0].def.nodes[1].style,
            "the copy's styles are the original's, unnormalised"
        );
    }

    #[test]
    fn mcp_figure_scale_grows_lengths_and_refuses_bad_factors() {
        let mut core = EditorCore::new();
        let before: f32 = core.project.figures[0]
            .def
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .map(|n| n.length)
            .sum();
        let out = tool_figure_scale(&mut core, &json!({ "figure": 0, "factor": 1.1 }))
            .expect("scale succeeds");
        assert_eq!(out["ok"], json!(true));
        let after: f32 = core.project.figures[0]
            .def
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .map(|n| n.length)
            .sum();
        assert!(after > before, "lengths grew: {before} -> {after}");
        assert!(tool_figure_scale(&mut core, &json!({ "figure": 0, "factor": 0.0 })).is_err());
        assert!(tool_figure_scale(&mut core, &json!({ "figure": 0, "factor": -1.0 })).is_err());
        assert!(tool_figure_scale(&mut core, &json!({ "figure": 7, "factor": 1.1 })).is_err());
    }

    #[test]
    fn mcp_copy_pose_between_copies_and_guards() {
        let mut core = EditorCore::new();
        core.add_figure();
        // Give figure 1 a distinct current-frame pose.
        core.project.frames[0].poses[1].angles[0] = 33.0;
        let want = core.project.frames[0].poses[1].angles.clone();
        let out = tool_copy_pose_between(
            &mut core,
            &json!({ "from_figure": 1, "to_figure": 0 }),
        )
        .expect("copy succeeds");
        assert_eq!(out["frame"], json!(0));
        assert_eq!(core.project.frames[0].poses[0].angles, want);
        assert!(tool_copy_pose_between(
            &mut core,
            &json!({ "from_figure": 0, "to_figure": 0 })
        )
        .is_err());
        // Rig mismatch: shorten figure 1's pose row.
        core.project.frames[0].poses[1].angles.truncate(2);
        let err = tool_copy_pose_between(
            &mut core,
            &json!({ "from_figure": 1, "to_figure": 0 }),
        )
        .expect_err("mismatched rigs are refused, not panicked on");
        assert!(err.contains("rig size"), "{err}");
    }

    #[test]
    fn mcp_reset_pose_and_apply_all_frames() {
        let mut core = EditorCore::new();
        core.project.frames[0].poses[0].angles[0] = 77.0;
        let rest0 = core.project.figures[0].def.nodes[0].rest_angle;
        let out = tool_reset_pose(&mut core, &json!({ "figure": 0, "frame": 0 }))
            .expect("reset succeeds");
        assert_eq!(out["pose"]["angles"][0], json!(rest0));
        assert_eq!(core.project.frames[0].poses[0].angles[0], rest0);

        let n = core.project.figures[0].def.nodes.len();
        let angles: Vec<f32> = vec![5.0; n];
        let out = tool_pose_apply_all_frames(
            &mut core,
            &json!({ "figure": 0, "angles": angles }),
        )
        .expect("apply-all succeeds");
        assert_eq!(out["applied"], json!(core.project.frames.len()));
        assert_eq!(out["skipped"], json!(0));
        assert!(core
            .project
            .frames
            .iter()
            .all(|f| f.poses[0].angles.iter().all(|a| *a == 5.0)));
        // A wrong-length pose applies nowhere and errors.
        assert!(tool_pose_apply_all_frames(
            &mut core,
            &json!({ "figure": 0, "angles": [1.0, 2.0] })
        )
        .is_err());
    }

    #[test]
    fn mcp_centre_and_ground_move_the_root() {
        let mut core = EditorCore::new();
        let out = tool_figure_ground(&mut core, &json!({ "figure": 0 }))
            .expect("ground succeeds");
        assert_eq!(out["ok"], json!(true));
        // After grounding, the drawn box bottom sits on
        // the stage's bottom edge.
        let bbox = fop_figure_bbox(&core, 0).expect("bbox exists");
        assert!(
            (bbox[3] - core.project.stage[1]).abs() < 0.5,
            "box bottom {} vs stage bottom {}",
            bbox[3],
            core.project.stage[1]
        );
        let out = tool_figure_centre(&mut core, &json!({ "figure": 0 }))
            .expect("centre succeeds");
        assert_eq!(out["ok"], json!(true));
        assert!(tool_figure_ground(&mut core, &json!({ "figure": 42 })).is_err());
    }

    #[test]
    fn mcp_layers_bulk_visibility_counts_changes() {
        let mut core = EditorCore::new();
        core.add_figure();
        let out = tool_layers_set_all_visible(&mut core, &json!({ "visible": false }))
            .expect("hide all succeeds");
        assert_eq!(out["changed"], json!(2));
        let out = tool_layers_set_all_visible(&mut core, &json!({ "visible": false }))
            .expect("hiding again is a no-change success");
        assert_eq!(out["changed"], json!(0));
        let out = tool_layers_invert_visibility(&mut core, &json!({}))
            .expect("invert succeeds");
        assert_eq!(out["changed"], json!(2));
        assert!(core.project.figures.iter().all(|f| !f.hidden));
    }

    #[test]
    fn mcp_layers_bulk_visibility_marks_dirty_only_for_actual_changes() {
        // Wave 13 (B3): the bulk visibility writes counted
        // an actual change through `mark_dirty` — exactly
        // once per call, however many figures flipped —
        // and a no-change call counted nothing. Roadmap
        // Wave 1 (RM24, P-14) keeps that counting shape but
        // the count is now an undo step: exactly one per
        // changing call, none for a no-change call.
        let mut core = EditorCore::new();
        core.add_figure();
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();

        tool_layers_set_all_visible(&mut core, &json!({ "visible": false }))
            .expect("hide all succeeds");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "one edit counted for the whole bulk write"
        );
        assert_eq!(core.undo_depth(), depth + 1, "one undo step for the bulk write");

        tool_layers_set_all_visible(&mut core, &json!({ "visible": false }))
            .expect("hiding again is a no-change success");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "a no-change call counts nothing"
        );
        assert_eq!(core.undo_depth(), depth + 1, "a no-change call spends no step");

        tool_layers_invert_visibility(&mut core, &json!({}))
            .expect("invert succeeds");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 2,
            "the invert flipped flags, so it counts once"
        );
        assert_eq!(core.undo_depth(), depth + 2, "the invert takes its own step");
    }

    #[test]
    fn fop_pose_library_round_trips_in_a_temp_dir() {
        assert_eq!(fop_pose_slug("Wave Pose!"), "wave-pose");
        assert_eq!(fop_pose_slug(""), "figure");
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-fop-pose-lib-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let core = EditorCore::new();
        let pose = core.project.frames[0].poses[0].clone();
        let path = fop_save_pose_library_in(&dir, &pose, "Wave Pose")
            .expect("save succeeds");
        assert!(path.ends_with("wave-pose.json"));
        // The file shape is the panel's: {"name", "pose"}.
        let text = std::fs::read_to_string(&path).expect("entry readable");
        let parsed: serde_json::Value =
            serde_json::from_str(&text).expect("entry is JSON");
        assert_eq!(parsed["name"], json!("Wave Pose"));
        assert!(parsed["pose"]["angles"].is_array());
        assert_eq!(fop_scan_pose_library_in(&dir), vec!["Wave Pose".to_string()]);
        let loaded = fop_load_pose_library_in(&dir, "Wave Pose").expect("load succeeds");
        assert_eq!(loaded.angles, pose.angles);
        assert_eq!(loaded.root, pose.root);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fop_pose_library_preserves_out_of_limit_angles_verbatim() {
        // Wave Z (Z9, library state round-trip audit): the
        // pose library stores a Pose {root, angles} only —
        // no states (they live on FigureInstance) and no
        // limits (they live on FigureDef) — and stored
        // poses are NEVER rewritten by limits (Wave X, P-4:
        // limits bind solving, not data). A pose authored
        // PAST the humanoid presets (elbow max +160, knee
        // min -160) must therefore round-trip through this
        // library bit-exact, unclamped.
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-fop-pose-limits-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let core = EditorCore::new();
        let mut pose = core.project.frames[0].poses[0].clone();
        pose.angles[4] = 170.0; // past the elbow preset max
        pose.angles[8] = -175.0; // past the knee preset min
        let path = fop_save_pose_library_in(&dir, &pose, "Over Limit")
            .expect("save succeeds");
        // The over-limit values are in the file itself.
        let text = std::fs::read_to_string(&path).expect("entry readable");
        let parsed: serde_json::Value =
            serde_json::from_str(&text).expect("entry is JSON");
        assert_eq!(parsed["pose"]["angles"][4], json!(170.0));
        assert_eq!(parsed["pose"]["angles"][8], json!(-175.0));
        let loaded = fop_load_pose_library_in(&dir, "Over Limit").expect("load succeeds");
        assert_eq!(loaded.angles, pose.angles, "angles round-trip unclamped");
        assert_eq!(loaded.root, pose.root);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fop_pose_entry_file_resolves_the_stored_name_not_the_slug() {
        // Wave W, W9 (mirror of the pose_tools pin): a
        // listed entry whose file stem is not its stored
        // name's slug still resolves — load, delete and
        // rename all address files through this resolver.
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-fop-pose-entry-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let core = EditorCore::new();
        let pose = core.project.frames[0].poses[0].clone();
        let canonical = fop_save_pose_library_in(&dir, &pose, "Wave Pose")
            .expect("save succeeds");
        let placed = dir.join("backup.json");
        std::fs::rename(&canonical, &placed).expect("external rename");
        assert_eq!(fop_scan_pose_library_in(&dir), vec!["Wave Pose".to_string()]);
        assert_eq!(fop_pose_entry_file_in(&dir, "Wave Pose"), placed);
        let loaded = fop_load_pose_library_in(&dir, "Wave Pose").expect("load succeeds");
        assert_eq!(loaded.angles, pose.angles);
        // App-saved entries resolve to their canonical
        // file; a name no file stores falls back to the
        // slug path (the historical error site).
        let canonical2 = fop_save_pose_library_in(&dir, &pose, "Styk")
            .expect("save succeeds");
        assert_eq!(fop_pose_entry_file_in(&dir, "Styk"), canonical2);
        assert_eq!(fop_pose_entry_file_in(&dir, "Ghost"), dir.join("ghost.json"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_figure_color_writes_the_figure() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.set_figure_color(fi, 0x2686E8);
        assert_eq!(core.project.figures[fi].color, 0x2686E8);
        assert!(core.status.contains("colour changed"));
    }

    #[test]
    fn set_figure_color_guards_a_stale_index() {
        let mut core = EditorCore::new();
        let before = core.project.figures[0].color;
        core.set_figure_color(999, 0x2686E8);
        assert_eq!(core.status, "No such figure.");
        assert_eq!(core.project.figures[0].color, before);
    }

    #[test]
    fn set_figure_color_marks_dirty_without_an_undo_step() {
        // Wave 13 (B3): the colour write counted toward
        // the dirty dot and autosave cadence through
        // `mark_dirty` while staying non-undoable. Roadmap
        // Wave 1 (RM24, P-14) closed the census: a real
        // change now takes a coalesced undo step — a run
        // of colour picks is ONE step that counts one edit
        // per pick.
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let stored = core.project.figures[fi].color;
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();

        core.set_figure_color(fi, 0x2686E8);
        assert_ne!(stored, 0x2686E8, "the test colour differs from the stored one");
        assert_eq!(core.edits_since_autosave(), edits + 1, "one edit counted for the change");
        assert_eq!(core.undo_depth(), depth + 1, "one coalesced step for the colour run");

        // A second, different pick folds into the same
        // step (the gesture is one undo).
        core.set_figure_color(fi, 0x123456);
        assert_eq!(core.edits_since_autosave(), edits + 2, "the fold still counts the edit");
        assert_eq!(core.undo_depth(), depth + 1, "the run stays one step");

        // Re-selecting the stored colour changes nothing
        // and spends nothing; a stale index spends nothing.
        core.set_figure_color(fi, 0x123456);
        core.set_figure_color(999, 0x123456);
        assert_eq!(core.edits_since_autosave(), edits + 2);
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn recolor_figure_colours_every_node_as_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let before: Vec<Option<u32>> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.color)
            .collect();
        assert!(before.len() > 1, "the demo figure has several nodes");
        let depth = core.undo_depth();
        assert!(core.recolor_figure(fi, 0xE8862E));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "exactly one snapshot for the whole figure"
        );
        assert!(core.project.figures[fi]
            .def
            .nodes
            .iter()
            .all(|n| n.color == Some(0xE8862E)));
        assert_eq!(core.status, format!("Figure {} recoloured.", fi + 1));
        // A single undo restores every node's original
        // colour — the proof the writes shared one snapshot.
        core.undo();
        let after: Vec<Option<u32>> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.color)
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn recolor_figure_keeps_gradient_end_colours() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].def.nodes[0].style.color2 = Some(0x112233);
        assert!(core.recolor_figure(fi, 0x445566));
        assert_eq!(core.project.figures[fi].def.nodes[0].color, Some(0x445566));
        assert_eq!(
            core.project.figures[fi].def.nodes[0].style.color2,
            Some(0x112233),
            "the gradient end colour is not part of a recolour"
        );
    }

    #[test]
    fn recolor_figure_guards_a_stale_index_without_a_snapshot() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        assert!(!core.recolor_figure(999, 0xE8862E));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "No figure to recolour.");
    }

    #[test]
    fn recolor_figure_refuses_a_figure_with_no_nodes() {
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes.clear();
        let depth = core.undo_depth();
        assert!(!core.recolor_figure(0, 0xE8862E));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "Figure has no segments to recolour.");
    }

    #[test]
    fn mirror_figure_all_frames_mirrors_every_frame_as_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        // Record every frame's pose for the figure before
        // the mirror; the demo project carries a pose row
        // for it on each frame.
        let before: Vec<Pose> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].clone())
            .collect();
        assert!(before.len() > 1, "the demo project has several frames");
        assert!(
            before.iter().all(|p| !p.angles.is_empty()),
            "the demo figure has angles to negate"
        );
        let depth = core.undo_depth();
        assert!(core.mirror_figure_all_frames(fi));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "exactly one snapshot for the whole figure"
        );
        for (frame, orig) in core.project.frames.iter().zip(&before) {
            let pose = &frame.poses[fi];
            assert_eq!(
                pose.root, orig.root,
                "mirroring negates angles only — the root stays put, as in mirror_selected_pose"
            );
            for (m, b) in pose.angles.iter().zip(&orig.angles) {
                assert_eq!(*m, -*b, "every angle on every frame is negated");
            }
        }
        assert_eq!(
            core.status,
            format!("Figure {} mirrored on all {} frames.", fi + 1, before.len())
        );
        // A single undo restores every frame's original
        // pose — the proof the writes shared one snapshot.
        core.undo();
        for (frame, orig) in core.project.frames.iter().zip(&before) {
            assert_eq!(frame.poses[fi].angles, orig.angles);
            assert_eq!(frame.poses[fi].root, orig.root);
        }
    }

    #[test]
    fn mirror_figure_all_frames_skips_frames_without_a_pose_row() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        // Make the pose table ragged: drop the figure's
        // pose row from the last frame.
        let last = core.project.frames.len() - 1;
        core.project.frames[last].poses.remove(fi);
        let before: Vec<Vec<f32>> = core
            .project
            .frames
            .iter()
            .map(|f| {
                f.poses
                    .get(fi)
                    .map(|p| p.angles.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert!(core.mirror_figure_all_frames(fi));
        assert!(
            core.project.frames[last].poses.get(fi).is_none(),
            "a frame with no pose row stays without one"
        );
        for (idx, frame) in core.project.frames.iter().enumerate() {
            if let Some(pose) = frame.poses.get(fi) {
                for (m, b) in pose.angles.iter().zip(&before[idx]) {
                    assert_eq!(*m, -*b);
                }
            }
        }
    }

    #[test]
    fn mirror_figure_all_frames_guards_a_stale_index_without_a_snapshot() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        assert!(!core.mirror_figure_all_frames(999));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "No figure to mirror.");
    }

    #[test]
    fn duplicate_figure_inserts_a_selected_copy_with_its_poses() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        let fi = core.selected_figure;
        let orig = core.project.figures[fi].clone();
        assert_eq!(core.duplicate_figure(fi), Some(fi + 1));
        assert_eq!(core.project.figures.len(), n + 1);
        let copy = &core.project.figures[fi + 1];
        assert_eq!(copy.name, format!("{} copy", orig.name));
        assert_eq!(copy.color, orig.color);
        assert_eq!(copy.def.nodes.len(), orig.def.nodes.len());
        assert_eq!(copy.opacity, orig.opacity);
        // The original is untouched and every frame's pose
        // table stays parallel to the figure list.
        assert_eq!(core.project.figures[fi].name, orig.name);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), core.project.figures.len());
        }
        if let Some(frame) = core.project.frames.first() {
            assert_eq!(
                frame.poses[fi + 1].angles,
                frame.poses[fi].angles,
                "the copy starts from the original's pose"
            );
        }
        assert_eq!(core.selected_figure, fi + 1);
        assert!(core.selected_node.is_none());
        assert_eq!(core.status, "Figure duplicated.");
    }

    #[test]
    fn duplicate_figure_without_figures_is_a_no_op() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(core.duplicate_figure(0), None);
        assert!(core.project.figures.is_empty());
        assert_eq!(core.status, "No figure to duplicate.");
    }

    #[test]
    fn duplicate_figure_guards_a_stale_index() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        assert_eq!(core.duplicate_figure(999), None);
        assert_eq!(core.project.figures.len(), n);
        assert_eq!(core.status, "No figure to duplicate.");
    }

    fn fx_entry(figure: usize) -> crate::model::FxClipEntry {
        crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 6.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFFCC33,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure,
            node: 0,
            hidden: false,
        }
    }

    fn channel(figure: usize) -> crate::graph::Channel {
        crate::graph::Channel {
            figure,
            target: crate::graph::ChannelTarget::Angle(0),
            keys: Vec::new(),
        }
    }

    fn particle(figure: Option<usize>) -> ParticleEvent {
        ParticleEvent {
            frame: 0,
            figure,
            node: None,
            x: 0.0,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 0,
        }
    }

    #[test]
    fn duplicate_figure_remaps_fx_channels_and_camera_follow() {
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        // Anchors on the later figure (shift), on the
        // duplicated figure itself (stay).
        core.project.fx_clips = vec![fx_entry(1), fx_entry(0)];
        core.project.channels = vec![channel(1), channel(0)];
        core.project.camera_follow = Some(1);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.fx_clips[1].figure, 0);
        assert_eq!(core.project.channels[0].figure, 2);
        assert_eq!(core.project.channels[1].figure, 0);
        assert_eq!(core.project.camera_follow, Some(2));
        // Following the duplicated figure itself stays put.
        core.project.camera_follow = Some(0);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.camera_follow, Some(0));
    }

    #[test]
    fn remap_after_remove_drops_dead_anchors_and_shifts_the_rest() {
        let mut project = Project::new_demo();
        let fig = project.figures[0].clone();
        project.figures.push(fig.clone());
        project.figures.push(fig);
        project.particles = vec![
            particle(Some(0)),
            particle(Some(1)),
            particle(Some(2)),
            particle(None),
        ];
        project.fx_clips = vec![fx_entry(0), fx_entry(1), fx_entry(2)];
        project.channels = vec![channel(0), channel(1), channel(2)];
        project.camera_follow = Some(2);
        remap_figures_after_remove(&mut project, 1);
        // Anchored to the removed figure: dropped everywhere.
        // Above it: shifted down. Below / unanchored: untouched.
        let pfigs: Vec<Option<usize>> = project.particles.iter().map(|e| e.figure).collect();
        assert_eq!(pfigs, vec![Some(0), Some(1), None]);
        let fxfigs: Vec<usize> = project.fx_clips.iter().map(|e| e.figure).collect();
        assert_eq!(fxfigs, vec![0, 1]);
        let chfigs: Vec<usize> = project.channels.iter().map(|c| c.figure).collect();
        assert_eq!(chfigs, vec![0, 1]);
        assert_eq!(project.camera_follow, Some(1));
        // Following the removed figure itself: follow turns off.
        project.camera_follow = Some(1);
        remap_figures_after_remove(&mut project, 1);
        assert_eq!(project.camera_follow, None);
    }

    #[test]
    fn remap_after_insert_shifts_at_and_above_only() {
        let mut project = Project::new_demo();
        project.particles = vec![particle(Some(0)), particle(Some(1))];
        project.fx_clips = vec![fx_entry(0), fx_entry(1)];
        project.channels = vec![channel(0), channel(1)];
        project.camera_follow = Some(0);
        remap_figures_after_insert(&mut project, 1);
        assert_eq!(project.particles[0].figure, Some(0));
        assert_eq!(project.particles[1].figure, Some(2));
        assert_eq!(project.fx_clips[0].figure, 0);
        assert_eq!(project.fx_clips[1].figure, 2);
        assert_eq!(project.channels[0].figure, 0);
        assert_eq!(project.channels[1].figure, 2);
        assert_eq!(project.camera_follow, Some(0));
    }

    #[test]
    fn duplicate_figure_shifts_particle_anchors_and_solo() {
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        // Anchors on the figure above the insertion point, on
        // the duplicated figure itself, and on no figure.
        for figure in [Some(1), Some(0), None] {
            core.project.particles.push(ParticleEvent {
                frame: 0,
                figure,
                node: None,
                x: 0.0,
                y: 0.0,
                kind: ParticleKind::Burst,
                color: 0xFFFFFF,
                seed: 0,
            });
        }
        core.solo = Some(1);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.particles[0].figure, Some(2));
        assert_eq!(core.project.particles[1].figure, Some(0));
        assert_eq!(core.project.particles[2].figure, None);
        assert_eq!(core.solo, Some(2));
    }

    #[test]
    fn remap_index_after_move_matches_the_move_permutation() {
        // Move 1 -> 3 in a five-figure project: the moved
        // figure lands at 3, figures 2 and 3 slide down one.
        let fwd: Vec<usize> = (0..5).map(|i| remap_index_after_move(i, 1, 3)).collect();
        assert_eq!(fwd, vec![0, 3, 1, 2, 4]);
        // The inverse move undoes it exactly.
        let back: Vec<usize> = fwd
            .iter()
            .map(|&i| remap_index_after_move(i, 3, 1))
            .collect();
        assert_eq!(back, vec![0, 1, 2, 3, 4]);
        // Adjacent swap: only the two neighbours exchange.
        let swap: Vec<usize> = (0..4).map(|i| remap_index_after_move(i, 2, 3)).collect();
        assert_eq!(swap, vec![0, 1, 3, 2]);
        // from == to is the identity, and a reference outside
        // the moved span is untouched either way.
        assert_eq!(remap_index_after_move(2, 2, 2), 2);
        assert_eq!(remap_index_after_move(9, 1, 3), 9);
        assert_eq!(remap_index_after_move(0, 1, 3), 0);
    }

    /// A four-figure project whose figures carry distinct
    /// names, with every census field referencing figures
    /// across the whole range (plus one unanchored particle),
    /// and frames whose poses are distinct per figure so any
    /// accidental frame write shows up in a snapshot compare.
    fn named_project() -> Project {
        let mut project = Project::new_demo();
        let proto = project.figures[0].clone();
        project.figures = ["alpha", "bravo", "charlie", "delta"]
            .iter()
            .map(|name| {
                let mut fig = proto.clone();
                fig.name = (*name).into();
                fig
            })
            .collect();
        for (fr, frame) in project.frames.iter_mut().enumerate() {
            frame.poses = (0..4)
                .map(|i| Pose {
                    root: [i as f32 * 10.0, fr as f32],
                    angles: vec![i as f32 + 0.5],
                })
                .collect();
        }
        project.particles = vec![
            particle(Some(0)),
            particle(Some(1)),
            particle(Some(2)),
            particle(Some(3)),
            particle(None),
        ];
        project.fx_clips = vec![fx_entry(3), fx_entry(0), fx_entry(2), fx_entry(1)];
        project.channels = vec![channel(1), channel(3), channel(0), channel(2)];
        project.camera_follow = Some(2);
        project
    }

    /// The NAME each stored reference resolves to, in field
    /// order — identity, not indices, so a test can prove a
    /// reference still names the same figure after a move.
    fn ref_names(
        project: &Project,
    ) -> (
        Vec<Option<String>>,
        Vec<String>,
        Vec<String>,
        Option<String>,
    ) {
        let name = |f: usize| project.figures[f].name.clone();
        (
            project
                .particles
                .iter()
                .map(|e| e.figure.map(&name))
                .collect(),
            project.fx_clips.iter().map(|e| name(e.figure)).collect(),
            project.channels.iter().map(|c| name(c.figure)).collect(),
            project.camera_follow.map(&name),
        )
    }

    /// (root, angles) of every pose in every frame.
    fn frame_snapshot(project: &Project) -> Vec<Vec<([f32; 2], Vec<f32>)>> {
        project
            .frames
            .iter()
            .map(|f| f.poses.iter().map(|p| (p.root, p.angles.clone())).collect())
            .collect()
    }

    #[test]
    fn remap_after_move_keeps_every_reference_on_its_figure() {
        // first->last, last->first, middle->middle, and both
        // directions of an adjacent swap.
        for (from, to) in [(0, 3), (3, 0), (1, 2), (2, 1), (2, 3)] {
            let mut project = named_project();
            let before = ref_names(&project);
            let frames_before = frame_snapshot(&project);
            // The caller's half of a move: reorder the
            // figures vec; the helper fixes the references.
            let fig = project.figures.remove(from);
            project.figures.insert(to, fig);
            remap_figures_after_move(&mut project, from, to);
            assert_eq!(
                ref_names(&project),
                before,
                "move {from} -> {to} must keep every reference on its figure"
            );
            assert_eq!(
                frame_snapshot(&project),
                frames_before,
                "the remap pass must not touch frame data"
            );
        }
        // The simulation itself: moving 0 -> 3 rotates the
        // vec, it does not swap the ends.
        let mut project = named_project();
        let fig = project.figures.remove(0);
        project.figures.insert(3, fig);
        let order: Vec<&str> = project.figures.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(order, vec!["bravo", "charlie", "delta", "alpha"]);
    }

    #[test]
    fn remap_after_move_no_ops_leave_references_untouched() {
        let raw_refs = |p: &Project| {
            (
                p.particles.iter().map(|e| e.figure).collect::<Vec<_>>(),
                p.fx_clips.iter().map(|e| e.figure).collect::<Vec<_>>(),
                p.channels.iter().map(|c| c.figure).collect::<Vec<_>>(),
                p.camera_follow,
            )
        };
        // from == to, target out of range, source out of
        // range: the raw indices must be byte-identical.
        for (from, to) in [(1, 1), (0, 9), (9, 0), (4, 4)] {
            let mut project = named_project();
            let before = raw_refs(&project);
            remap_figures_after_move(&mut project, from, to);
            assert_eq!(raw_refs(&project), before, "move {from} -> {to} is a no-op");
        }
    }
}
