//! Pose tools section for the Rune editor shell (Phase D,
//! section 30/40): mirror / copy / paste / copy-between /
//! reset / graph-bake controls for the selected figure's pose
//! on the current frame. Declared from `main.rs` only — bin
//! crate code, not part of the engine library.
//!
//! INTEGRATOR NOTE — one new `crate::State` field is required
//! (this file does not compile against `State` without it):
//!
//! ```ignore
//! pub pose_clipboard: Option<stykanimate_rs::model::Pose>,
//! ```
//!
//! initialised to `None` in `init()`. The core's own clipboard
//! (`EditorCore::clipboard`, editor.rs:53) is private and holds
//! a whole `Frame`; a pose-scoped clipboard has nowhere to live
//! but the shell state — the same reason `State::wheel_zoom`
//! exists for the camera gesture.
//!
//! Phase G adds a second field for the pose-library rename
//! session (see [`PoseRenameState`]):
//!
//! ```ignore
//! pub pose_rename: rune_sec::pose_tools::PoseRenameState,
//! ```
//!
//! initialised with `PoseRenameState::new()` in `init()`,
//! plus one line in `main.rs`'s `text_focused()`:
//!
//! ```ignore
//! || s.pose_rename.field.focused
//! ```
//!
//! The field cannot live in this file's thread-local cache:
//! `text_input`'s `on_edit` needs a `&mut TextEdit` path into
//! `State` (rune's el/text.rs `on_edit` signature), the same
//! wall the movieclips rename hit (clips_panel's module
//! docs). Every full `State { … }` literal in the shell's
//! test modules needs the new field too, exactly as the
//! Phase F fields were patched in.
//!
//! Phase I adds a third field for the pose-library name
//! filter (see [`PoseLibraryState`]), on the same terms:
//!
//! ```ignore
//! pub pose_library: rune_sec::pose_tools::PoseLibraryState,
//! ```
//!
//! initialised with `PoseLibraryState::new()` in `init()`,
//! plus one line in `main.rs`'s `text_focused()`:
//!
//! ```ignore
//! || s.pose_library.filter.focused
//! ```
//!
//! Control → behaviour map (every control is real; there is
//! no placeholder in this section):
//!
//! - **Mirror pose** → `EditorCore::mirror_selected_pose`
//!   (editor.rs:1405). This is also the brief's "Flip figure":
//!   no separate flip op exists anywhere in the engine
//!   (verified by grep over editor.rs / fig3d.rs / model.rs),
//!   and mirroring *is* the horizontal flip — it negates every
//!   angle about the vertical axis through the root
//!   (editor.rs:1390-1404). Two buttons calling one op would
//!   be a fake distinction, so there is one button.
//! - **Copy pose** → composed: stores the selected figure's
//!   current-frame pose in `State::pose_clipboard`. No core op
//!   copies a single pose to a clipboard; the body follows
//!   `EditorCore::copy_frame`'s clipboard pattern
//!   (editor.rs:1655) scoped to one pose. Not an undo step,
//!   exactly like `copy_frame`.
//! - **Paste pose** → composed: `EditorCore::snapshot`
//!   (editor.rs:202) + write the clipboard pose over the
//!   selected figure's current-frame pose — one undo step,
//!   mirroring `copy_pose_between`'s structure. Guarded on
//!   angle count: `model::evaluate` indexes `pose.angles[i]`
//!   directly (model.rs:326-330), so pasting a pose from a
//!   differently-rigged figure would panic the renderer; the
//!   paste refuses with a status message instead.
//! - **Mirror figure (all frames)** (Phase J) →
//!   `EditorCore::mirror_figure_all_frames(selected)`
//!   (figure_ops.rs): the single-frame mirror applied to
//!   the figure's pose on EVERY frame under one snapshot,
//!   so one undo restores the whole timeline. Frames with
//!   no pose row for the figure are skipped. The core
//!   op's graph-channel limit carries over: channels are
//!   not mirrored, so a channel-driven figure's evaluated
//!   poses will not flip even though its stored poses do.
//! - **Apply pose to all frames** (Phase J) → composed
//!   here ([`apply_pose_to_all_frames`]): the clipboard
//!   pose written over the selected figure's pose on
//!   every frame as ONE undo step. Calling [`paste_pose`]
//!   per frame would take one snapshot per frame
//!   (`write_current_pose` snapshots on every call), so
//!   the batch plans first — a frame is writable when it
//!   holds a pose row for the figure whose angle count
//!   matches the clipboard's, exactly Paste pose's two
//!   guards judged per frame — then snapshots once and
//!   makes `write_current_pose`'s own assignment
//!   (`project.frames[f].poses[fi] = pose`) per writable
//!   frame, addressed by index so the current frame
//!   never moves. Skipped frames are counted in the
//!   status, never silently passed over. A partial
//!   application (some frames skipped) is worded in the
//!   status classifier's Warn grammar; only a full
//!   application wears the plain shape (Wave 10).
//! - **Copy pose to…** (one button per other figure) →
//!   `EditorCore::copy_pose_between(selected, target)`
//!   (editor.rs:1597), reading the selection live at press
//!   time, routed through [`copy_pose_to`]: the engine op
//!   itself has NO angle-count check (it writes the source
//!   pose row over the target's unconditionally), so the
//!   guard Paste pose carries is judged across figures
//!   here first — a mismatched copy would panic
//!   `model::evaluate` on the next render.
//! - **Reset pose** → composed: `snapshot` + replace the
//!   current pose with `FigureDef::rest_pose(current root)`
//!   (model.rs:279) — the same rest pose a figure is created
//!   with (`Project::add_figure_with`, model.rs:976). The root
//!   position is kept; only the angles reset.
//! - **Bake graph** → `EditorCore::bake_graph_channels(0,
//!   last)` (editor.rs:1360) over the whole timeline — the op
//!   is pose-adjacent: it writes graph-evaluated poses into
//!   real frames, after which they are ordinary editable
//!   poses.
//! - **Save pose** (pose library, Phase F) → writes the
//!   selected figure's current pose to the on-disk library
//!   `./library/poses/<slug>.json`, mirroring the 3D panel's
//!   library (`fig3d::save_pose_library`, fig3d.rs:1544)
//!   convention for convention: same `{name, pose}` file
//!   envelope, same slug sanitising, same save/scan/load
//!   `_in` split, the 2D directory standing where
//!   `./library3d/poses` stands for 3D. The save name is the
//!   figure's display name, so saving the same figure again
//!   OVERWRITES its earlier entry — one library slot per
//!   figure name (the 3D library's collision behaviour too).
//! - **Pose library Apply buttons** → load a saved entry and
//!   write it over the selected figure's current-frame pose
//!   through the exact path Paste pose uses (one `snapshot`,
//!   one write, the same angle-count refusal — a pose from a
//!   differently-rigged figure would panic
//!   `model::evaluate`). The saved-name list is read through
//!   an mtime-keyed thread-local cache (the fx_browser /
//!   home_panel pattern): a save bumps the directory mtime,
//!   so a new entry appears on the very next build.
//! - **Pose library Mirror-apply buttons** (Phase G) →
//!   load a saved entry, mirror the loaded *value* with
//!   [`mirror_pose`] (negate every angle, root untouched —
//!   the exact mutation `EditorCore::mirror_selected_pose`
//!   performs, editor.rs:1450-1452), then write through the
//!   same [`write_current_pose`] path: exactly one undo
//!   step. Calling the core op *after* an apply would
//!   snapshot a second time (editor.rs:1448), so the mirror
//!   is composed on the value, before the write. The core
//!   op's own caveat carries over: bends, lengths and graph
//!   channels live outside `Pose` and are not mirrored, so
//!   the result is an exact mirror of the angle data, and
//!   of the rendered figure for bilaterally symmetric rigs.
//! - **Pose library Delete buttons** (Phase G) → remove
//!   the entry's file with `std::fs::remove_file`. A file
//!   operation, NOT an undo step — the same discipline as
//!   Save. A failed removal leaves the file (and so the
//!   listing) untouched and reports the error; a
//!   successful one moves the directory mtime, which is
//!   what refreshes the name cache, exactly as after a
//!   Save.
//! - **Pose library Rename buttons** (Phase G) → a small
//!   session ([`PoseRenameState`], the library panel's
//!   Phase F rename shape) edits an entry's display name:
//!   the new entry file (new slug, envelope name updated)
//!   is written FIRST and the old file removed second —
//!   the library panel's file-first discipline; the
//!   listing here is a directory scan, so it follows the
//!   disk on the next build. Refusals are decided before
//!   any disk write: blank name, a name another entry
//!   already owns (by name OR by slug — two display names
//!   that slug alike share one file), or a target file
//!   that exists but the scan could not read as a pose.
//!
//! Buttons whose op would no-op on the current state render
//! muted (secondary text, no press handler — inert, not fake):
//! mirror/copy/reset need a pose on this frame, paste needs a
//! copied pose *and* a target pose, mirror-all shares
//! mirror's gate and apply-all shares paste's, copy-to needs
//! both poses, bake needs at least one graph channel and one
//! frame.
//! Library apply and mirror-apply need a pose to land on;
//! library delete/rename act on files, not on the frame, so
//! they are always live.
//!
//! Phase H (section 17/40) puts Font Awesome glyphs on
//! the library controls through
//! `crate::rune_sec::icon_btn::icon_btn`: Save pose
//! (`floppy-disk`), Apply (`check`), Apply mirrored
//! (`copy` — the catalogue carried no flip glyph, and
//! `repeat` already means loop/cycle elsewhere in the
//! shell), Rename (`pencil`), Del (`trash-can`), and
//! the rename editor's Apply / Cancel (`check` /
//! `xmark`). The gated buttons (save, apply,
//! mirror-apply) keep the muted-inert rendering when
//! their op would no-op, glyph included, via `pibtn`;
//! the pose-op buttons (mirror / copy / paste / reset /
//! bake, copy-to) are not in this section's icon map and
//! stay on `pbtn`. Keys and handlers are unchanged.
//!
//! Phase I (section 23/40) retires the local half of
//! that arrangement: `pibtn`'s inert branch is now
//! icon_btn's shared inert twin
//! (`crate::rune_sec::icon_btn::icon_btn_inert`,
//! section 11) instead of a hand-rolled muted row —
//! same box metrics, muted glyph and label, no press
//! handler; the resting fill is the shared inert fill
//! (the card colour) rather than this file's old
//! `control_off`. Apply mirrored's glyph becomes
//! `arrow-right-arrow-left` ([`APPLY_MIRRORED_ICON`]):
//! the catalogue gained the honest flip glyph
//! (section 10), so the `copy` stand-in is retired.
//! The library also gains a name filter (field key
//! `pose-lib-filter`, state in [`PoseLibraryState`])
//! on figures_panel's Phase G idiom: the list shows
//! only entries matching the query, rows keep their
//! REAL indices into the scanned list (so every row
//! key and handler addresses the same entry, filtered
//! or not), and the count beside the heading is the
//! bare total while idle, "{shown} of {total} poses"
//! while a filter is active.
//!
//! Phase J (section 20/40) adds the two whole-timeline
//! pose actions beside their single-frame parents:
//! Mirror figure (all frames) sits next to Mirror pose,
//! and Apply pose to all frames next to Paste pose —
//! the first button row now holds the mirror pair, the
//! second the copy / paste / apply family. Both are
//! plain `pbtn`s: the pose-op buttons were never in
//! this section's icon map, and the all-frames pair
//! follows them. No new `State` field: both handlers
//! work through `core` and the existing
//! `State::pose_clipboard`.
//!
//! Phase N (adoption) restyles the panel in the
//! charter's card language and retires the two local
//! button dialects: the root is a kit
//! `design::panel_col` card with a `panel_title`
//! header and hairline divider; `pbtn` is deleted —
//! its live sites call `chrome_view::btn` and its
//! inert sites `chrome_view::btn_inert` (the same
//! no-handler inert discipline `pbtn` documented,
//! in the family's one implementation; a snap test
//! pins that an inert press writes no status);
//! `pibtn` is deleted — its call sites call
//! `icon_btn` / `icon_btn_inert` directly, as they
//! already dispatched. Library entries become the
//! two-line list-row shells (kit px / radius /
//! selected fill / hover on a `col`), the rename
//! target now carrying the selected fill (the
//! library panel's convention); the section labels
//! ("Pose library", "Copy pose to") come from
//! `design::section_label`, and the filter's
//! no-match line is a kit `empty_state`. Keys,
//! labels and handlers are byte-identical throughout.
//!
//! UI mission Wave 4 (audit S07-8a): the library
//! **Del** button — the app's only irreversible
//! delete (a `remove_file`, not an undo step) —
//! renders through `danger_icon_btn`: same call
//! shape, key, glyph and handler as the `icon_btn`
//! it replaces, danger fills instead of the plain
//! ones, so it no longer carries the same visual
//! weight as the Rename beside it. No two-step
//! confirm was added: neither library state struct
//! holds an armed/pending field, and this panel
//! has no confirm idiom to reuse. The same wave
//! closes the clipboard family's dialect split:
//! **Copy pose** (`copy`), **Paste pose**
//! (`clipboard` — the catalogue carries no `paste`
//! glyph) and **Apply pose to all frames** (`check`,
//! the library Apply buttons' glyph) now render
//! through `icon_btn` / `icon_btn_inert` like the
//! library controls below them, instead of the
//! glyph-less `btn` / `btn_inert`. Keys, labels,
//! gating and handlers are byte-identical; the row
//! already wraps (Wave 1), so the glyphs' width is
//! absorbed by the wrap.
//!
//! UI mission Wave 4 batch 2 (audit S07-12): the
//! library section no longer vanishes when the
//! library is empty — the header (`pose-lib-header`,
//! label + count) renders always, and an empty
//! library shows the kit `empty_state` ("shapes",
//! "No saved poses yet — Save pose stores the
//! current pose here.") under it, so the panel
//! itself teaches that Save pose is what fills the
//! list; the filter and entries render only when
//! entries exist. The filter's search glyph drops
//! 14 → 13 pt, the kit's search size (the figures
//! filter's glyph). Keys and handlers unchanged.
//!
//! UI mission Wave 10 (audit D7, pose tools round 2):
//! two findings land. **Copy pose to…** now routes
//! through [`copy_pose_to`] — the engine op it calls
//! has no angle-count check, so this row was the one
//! cross-figure path that could write a mismatched
//! pose into a row (and panic `model::evaluate` on
//! the next render, or leave unread angles sitting in
//! it); it now carries the guard Paste pose and the
//! library applies already had, judged across
//! figures. And **Apply pose to all frames** reports
//! a PARTIAL application in the classifier's Warn
//! grammar ("…; N cannot take the copied pose …") —
//! the rename flow's partial-failure precedent in
//! this same file — while a full application keeps
//! its plain shape and the zero-frame case already
//! read as a refusal (Wave 9). The rest of the
//! round-2 hunt audited clean, and the proofs are
//! pinned by tests below: the library is plain files
//! under `./library/poses` and nothing in the
//! project file references it, so saved poses
//! survive restarts but belong to the working
//! directory, not to any one project; a saved pose
//! is root + angles only, so applying it to a
//! same-count figure with different proportions
//! transfers the angles exactly and leaves the
//! target's own segment lengths in place (no silent
//! distortion — a different angle count is the
//! refusal case); the pose clipboard is a third,
//! separate clipboard beside the core's frame and
//! range clipboards, written by nothing outside
//! this file, so neither paste shadows the other;
//! and rename/delete already refuse the blank-name,
//! exact- and slug-collision, and unreadable-target
//! cases (the FX presets refuse their empty-slug
//! case at save/load and have no rename at all).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El, TextEdit, Tx};
use serde::{Deserialize, Serialize};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn, icon_btn_inert};
use crate::rune_sec::icons::icon_el;
use crate::State;

/// True when figure `fi` has a pose on the current frame —
/// the same existence check `mirror_selected_pose` performs
/// before acting (editor.rs:1411-1416).
pub fn pose_exists(core: &EditorCore, fi: usize) -> bool {
    core.project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .is_some()
}

/// The selected figure's pose on the current frame, if any.
pub fn selected_pose(core: &EditorCore) -> Option<&Pose> {
    core.project
        .frames
        .get(core.current)?
        .poses
        .get(core.selected_figure)
}

/// True when the selected figure has a pose here — the gate
/// for Mirror / Copy / Reset.
pub fn has_selected_pose(core: &EditorCore) -> bool {
    pose_exists(core, core.selected_figure)
}

/// Indices of every figure except the selected one — the
/// targets for Copy-pose-to. Empty for a one-figure project.
pub fn pose_targets(core: &EditorCore) -> Vec<usize> {
    (0..core.project.figures.len())
        .filter(|&i| i != core.selected_figure)
        .collect()
}

/// True when Bake graph would do work: the op no-ops with a
/// status message when there are no channels (editor.rs:1364)
/// or no frames (editor.rs:1368).
pub fn can_bake(core: &EditorCore) -> bool {
    !core.project.channels.is_empty() && core.frame_count() > 0
}

/// The selected figure's rest pose at its current root —
/// what Reset pose restores. `None` when there is no pose
/// here (or no such figure).
pub fn rest_pose_for(core: &EditorCore) -> Option<Pose> {
    let pose = selected_pose(core)?;
    let fig = core.project.figures.get(core.selected_figure)?;
    Some(fig.def.rest_pose(pose.root))
}

/// The status text the core itself uses when a pose op finds
/// nothing to act on — `mirror_selected_pose`'s two messages
/// (editor.rs:1408-1424), reused so the composed ops speak
/// with the same voice.
fn no_pose_status(core: &EditorCore) -> String {
    if core.selected_figure >= core.project.figures.len() {
        "No such figure.".to_string()
    } else {
        format!(
            "Figure {} has no pose on frame {}.",
            core.selected_figure + 1,
            core.current + 1
        )
    }
}

/// Display label for figure `j`: its project name, falling
/// back to `Figure {n}` when the name is blank.
fn figure_label(core: &EditorCore, j: usize) -> String {
    match core.project.figures.get(j) {
        Some(f) if !f.name.trim().is_empty() => f.name.clone(),
        _ => format!("Figure {}", j + 1),
    }
}

/// Copy the selected figure's current-frame pose into the
/// shell clipboard. See the module docs for why this is a
/// composition and not a core op.
pub fn copy_pose(core: &mut EditorCore, clip: &mut Option<Pose>) {
    let Some(pose) = selected_pose(core).cloned() else {
        core.status = no_pose_status(core);
        return;
    };
    let (fi, fr) = (core.selected_figure + 1, core.current + 1);
    *clip = Some(pose);
    core.status = format!("Pose copied from figure {fi} on frame {fr}.");
}

/// Paste a previously copied pose onto the selected figure
/// on the current frame, as one undo step. Refuses (status
/// only) when nothing was copied, when there is no target
/// pose, or when the copied pose's angle count does not
/// match this figure's rig — `model::evaluate` indexes
/// angles directly, so a mismatched paste would panic
/// instead of merely looking wrong.
pub fn paste_pose(core: &mut EditorCore, clip: &Option<Pose>) {
    let Some(clip) = clip else {
        core.status = "Nothing copied yet — copy a pose first.".into();
        return;
    };
    let Some(cur) = selected_pose(core) else {
        core.status = no_pose_status(core);
        return;
    };
    if clip.angles.len() != cur.angles.len() {
        core.status = format!(
            "Paste pose failed: copied pose has {} angles, figure {} has {} segments.",
            clip.angles.len(),
            core.selected_figure + 1,
            cur.angles.len()
        );
        return;
    }
    let clip = clip.clone();
    let (fi, fr) = (core.selected_figure, core.current);
    write_current_pose(core, clip);
    core.status = format!(
        "Pose pasted onto figure {} on frame {}.",
        fi + 1,
        fr + 1
    );
}

/// Copy-pose-to press for target figure `j`: Paste
/// pose's angle-count guard judged across figures, in
/// front of the engine op. The write itself is a core
/// op (`EditorCore::copy_pose_between`), and that op
/// has NO rig check — it clones the source pose row
/// over the target's unconditionally. The panel's
/// gate only proves both pose rows exist, so without
/// this guard the row was the one cross-figure path
/// that could store a mismatched pose: with fewer
/// angles than the target has nodes,
/// `model::evaluate`'s direct `pose.angles[i]`
/// indexing panics on the next render; with more,
/// the extra angles sit in the row unread. The
/// refusal is worded after Paste's, in the core op's
/// own "Copy pose failed: …" failure shape. Every
/// other outcome (missing rows, same figure) is the
/// core op's own status, reached by delegating.
pub fn copy_pose_to(core: &mut EditorCore, j: usize) {
    let counts = selected_pose(core).map(|p| p.angles.len()).zip(
        core.project
            .frames
            .get(core.current)
            .and_then(|f| f.poses.get(j))
            .map(|p| p.angles.len()),
    );
    if let Some((src_len, tgt_len)) = counts {
        if src_len != tgt_len {
            core.status = format!(
                "Copy pose failed: figure {} has {} angles, figure {} has {} segments.",
                core.selected_figure + 1,
                src_len,
                j + 1,
                tgt_len
            );
            return;
        }
    }
    core.copy_pose_between(core.selected_figure, j);
}

/// Snapshot once and write `pose` over the selected figure's
/// current-frame pose — the single write path Paste pose and
/// library Apply share, so both are exactly one undo step.
/// Callers guard pose existence and angle count first.
fn write_current_pose(core: &mut EditorCore, pose: Pose) {
    core.snapshot();
    let (fi, fr) = (core.selected_figure, core.current);
    core.project.frames[fr].poses[fi] = pose;
}

/// Apply the clipboard pose to the selected figure on
/// EVERY frame, as one undo step (Phase J). `paste_pose`
/// cannot be looped per frame: its write path,
/// [`write_current_pose`], snapshots on every call, so a
/// per-frame loop would bury the action under one undo
/// step per frame. Instead the batch plans first — a
/// frame is writable when it holds a pose row for the
/// figure whose angle count matches the clipboard's,
/// which is Paste pose's pair of guards (a pose row
/// exists; the angle count matches, or `model::evaluate`
/// would panic on the mismatched rig) judged per frame —
/// then takes ONE snapshot and makes the write path's
/// own assignment per writable frame, addressed by frame
/// index, so the current frame never moves. Frames that
/// fail either guard are skipped and counted in the
/// status. When nothing is writable there is no snapshot
/// and no change — the refusal discipline every other
/// composed op here follows.
pub fn apply_pose_to_all_frames(core: &mut EditorCore, clip: &Option<Pose>) {
    let Some(clip) = clip else {
        core.status = "Nothing copied yet — copy a pose first.".into();
        return;
    };
    if core.selected_figure >= core.project.figures.len() {
        core.status = no_pose_status(core);
        return;
    }
    let fi = core.selected_figure;
    let mut writable: Vec<usize> = Vec::new();
    let mut skipped = 0usize;
    for (f, frame) in core.project.frames.iter().enumerate() {
        match frame.poses.get(fi) {
            Some(p) if p.angles.len() == clip.angles.len() => writable.push(f),
            _ => skipped += 1,
        }
    }
    if writable.is_empty() {
        // A zero-frame apply changed nothing, so it must
        // not wear the success sentence's shape: worded as
        // a refusal, the status classifier reads it as a
        // warning (the "No …" family), not a neutral
        // confirmation (Wave 9 audit D7).
        core.status =
            format!("No frame could take the copied pose ({skipped} skipped).");
        return;
    }
    let clip = clip.clone();
    let applied = writable.len();
    core.snapshot();
    for f in writable {
        core.project.frames[f].poses[fi] = clip.clone();
    }
    core.status = if skipped == 0 {
        format!("Pose applied to {applied} frames (0 skipped).")
    } else {
        // A partial application — the user asked for
        // ALL frames and some could not take the pose —
        // must not wear the plain confirmation's shape:
        // "cannot" puts it in the status classifier's
        // Warn grammar, the same move the rename flow
        // below makes for its partial failure (Wave 10
        // audit D7; the zero-frame case above is the
        // full-refusal sibling, Wave 9).
        format!(
            "Pose applied to {applied} frames; {skipped} cannot take the copied pose (no pose row or a different rig)."
        )
    };
}

/// Reset the selected figure's current-frame pose to its
/// rest pose (root position kept), as one undo step.
pub fn reset_pose(core: &mut EditorCore) {
    let Some(rest) = rest_pose_for(core) else {
        core.status = no_pose_status(core);
        return;
    };
    core.snapshot();
    let (fi, fr) = (core.selected_figure, core.current);
    core.project.frames[fr].poses[fi] = rest;
    core.status = format!(
        "Figure {} reset to its rest pose on frame {}.",
        fi + 1,
        fr + 1
    );
}


// ----------------------------------------------------------------
// Pose library (Phase F): the clipboard's persistent sibling.
// Mirrors the 3D pose library in `fig3d` (fig3d.rs:1497-1597)
// convention for convention — see the module docs.
// ----------------------------------------------------------------

/// Sanitise a display name into a file-safe slug: lowercase
/// alphanumerics kept, every other run collapsed to one `-`,
/// no leading or trailing dash, `"figure"` when nothing
/// usable remains. A verbatim mirror of `fig3d::library_slug`
/// (fig3d.rs:1412), which is private to the engine crate;
/// like `fx_browser::PRESET_DIR`, this copy is the one
/// maintenance hazard — if the 3D slug rules ever change,
/// this must change with them, or the two libraries' files
/// stop following one convention.
pub fn pose_slug(name: &str) -> String {
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

/// The file stem the pose of a figure called `figure_name`
/// is stored under: the slug itself (the file is
/// `<stem>.json` in the pose library directory).
pub fn pose_file_stem(figure_name: &str) -> String {
    pose_slug(figure_name)
}

/// File name a library pose called `name` is stored under:
/// `<slug>.json`, where the slug comes from [`pose_slug`] —
/// the same sanitising the 3D library applies (mirrors
/// `fig3d::pose_file_name`).
fn pose_file_name(name: &str) -> String {
    format!("{}.json", pose_slug(name))
}

/// One pose library file's contents: a pose under the name
/// it was saved by (mirrors `fig3d::PoseLibraryEntry` for
/// the 3D library).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PoseLibraryEntry {
    name: String,
    pose: Pose,
}

/// The directory the 2D pose library lives in: the
/// `poses` folder inside the one resolver's library dir
/// (`stykanimate_rs::shell::library_dir`, Roadmap Wave 1,
/// RM9 / F-8) — where the 3D mirror keeps its `poses`
/// folder inside the library3d dir.
fn pose_library_dir() -> PathBuf {
    stykanimate_rs::shell::library_dir().join("poses")
}

/// Save `pose` into pose library directory `dir` (created
/// if missing) under `name`, returning the file's path. The
/// file is `<dir>/<slug>.json` (see [`pose_file_name`])
/// holding `{name, pose}` as JSON. Saving under a name that
/// already has an entry overwrites that file.
fn save_pose_library_in(
    dir: &Path,
    pose: &Pose,
    name: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(pose_file_name(name));
    let entry = PoseLibraryEntry {
        name: name.to_string(),
        pose: pose.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Save `pose` into the 2D pose library (`./library/poses`)
/// under `name`, returning the file's path. See
/// [`save_pose_library_in`].
pub fn save_pose_library(pose: &Pose, name: &str) -> std::io::Result<PathBuf> {
    save_pose_library_in(&pose_library_dir(), pose, name)
}

/// The file backing the pose entry displayed as `name` in
/// directory `dir`: the file whose stored name IS `name`,
/// found by the same scan [`scan_pose_library_in`] lists
/// from — the listing reads names out of file contents, so
/// resolution must read them too (a hand-placed or
/// externally renamed file whose stem is not its name's
/// slug is listed, and only this resolution finds it
/// again). When no file stores `name`, the historical slug
/// path is returned, so loading by a case/slug variant
/// keeps working and honest fs errors land on the
/// canonical path. Duplicate stored names resolve to the
/// first file in sorted path order. Mirrors
/// `fop_pose_entry_file_in` in figure_ops.rs — the two
/// copies change together (Wave W, W9).
fn pose_entry_file_in(dir: &Path, name: &str) -> PathBuf {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> =
            entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let is_pose_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".json"))
                .unwrap_or(false);
            if is_pose_file {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) =
                        serde_json::from_str::<PoseLibraryEntry>(&text)
                    {
                        if parsed.name == name {
                            return path;
                        }
                    }
                }
            }
        }
    }
    dir.join(pose_file_name(name))
}

/// Load the pose saved under `name` from pose library
/// directory `dir`. An error is returned when no readable,
/// well-formed entry exists for that name (missing file,
/// invalid JSON, wrong shape). The name resolves through
/// [`pose_entry_file_in`], so an entry the scan lists
/// always loads (see there).
fn load_pose_library_in(dir: &Path, name: &str) -> std::io::Result<Pose> {
    let path = pose_entry_file_in(dir, name);
    let text = std::fs::read_to_string(&path)?;
    let entry: PoseLibraryEntry =
        serde_json::from_str(&text).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
    Ok(entry.pose)
}

/// Load the pose saved under `name` from the 2D pose
/// library (`./library/poses`). See [`load_pose_library_in`].
pub fn load_pose_library(name: &str) -> std::io::Result<Pose> {
    load_pose_library_in(&pose_library_dir(), name)
}

/// The display names of every pose saved in pose library
/// directory `dir`, sorted. Files that are not readable
/// pose entries are skipped; a missing directory yields an
/// empty list.
fn scan_pose_library_in(dir: &Path) -> Vec<String> {
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
                    if let Ok(parsed) =
                        serde_json::from_str::<PoseLibraryEntry>(&text)
                    {
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

/// The display names of every pose saved in the 2D pose
/// library (`./library/poses`), sorted. See
/// [`scan_pose_library_in`].
pub fn scan_pose_library() -> Vec<String> {
    scan_pose_library_in(&pose_library_dir())
}

/// A directory's modification time, swallowing every error
/// into `None` — `fx_browser`'s helper, same rule: a
/// missing/unreadable dir is a state, not a failure
/// (`scan_pose_library` yields an empty list for it).
/// Callers treat `None` as "no trustworthy cache key", not
/// as a key.
fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// One pose-name cache slot: the dir-mtime key the names
/// were read under, plus the names themselves.
type PoseNameCache = Option<(Option<SystemTime>, Vec<String>)>;

thread_local! {
    /// The saved-pose name cache (fx_browser's design).
    /// Thread-local because Rune builds the view and runs
    /// its handlers on the UI thread (so this panel's own
    /// Save press and the next build share one cache), and
    /// because every test thread then starts with an empty
    /// cache.
    static POSE_NAME_CACHE: RefCell<PoseNameCache> = RefCell::new(None);
}

/// The cache state machine, pure over its inputs so tests
/// can drive it with a counting reader: serve the stored
/// names when `key` matches the key they were read under,
/// otherwise call `read`, store its result under `key`, and
/// return it. (fx_browser's `cached_names`, same contract.)
fn cached_pose_names(
    cache: &mut PoseNameCache,
    key: Option<SystemTime>,
    read: impl FnOnce() -> Vec<String>,
) -> Vec<String> {
    if let Some((cached_key, names)) = cache.as_ref() {
        if *cached_key == key {
            return names.clone();
        }
    }
    let names = read();
    *cache = Some((key, names.clone()));
    names
}

/// The saved pose names for the panel, cached behind the
/// library directory's mtime (the fx_browser contract: the
/// cache can only go stale on NAMES, and a save through
/// this panel's own button bumps the directory mtime, so a
/// new entry is visible on the very next build). Falls back
/// to a live scan — caching nothing — when the directory
/// cannot be statted (usually: nothing saved yet, so the
/// directory doesn't exist; the live scan returns the same
/// empty list the cache would hold).
pub fn saved_pose_names() -> Vec<String> {
    let Some(key) = dir_mtime(&pose_library_dir()) else {
        return scan_pose_library();
    };
    POSE_NAME_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        cached_pose_names(&mut slot, Some(key), scan_pose_library)
    })
}

/// Pose-library panel state (Phase I): only the name
/// filter's text. Like [`PoseRenameState`] this cannot
/// live in a thread-local — `text_input`'s `on_edit`
/// needs a `&mut TextEdit` path into `State` — so the
/// integrator homes it in `State` beside `pose_rename`;
/// the exact lines are in this file's module docs.
/// Filtering is view-only: it never touches the
/// library on disk or the project.
pub struct PoseLibraryState {
    /// Name-filter box contents (Rune core text edit).
    /// The entry list filters through
    /// [`visible_pose_entries`] on this text every
    /// render; typing is the only writer.
    pub filter: TextEdit,
}

impl PoseLibraryState {
    pub fn new() -> Self {
        PoseLibraryState {
            filter: TextEdit::new(""),
        }
    }
}

/// True when the filter `query` actually narrows the
/// list: non-empty once trimmed. A whitespace-only
/// query is no filter at all (figures_panel's
/// `filter_active`, same rule).
pub fn pose_filter_active(query: &str) -> bool {
    !query.trim().is_empty()
}

/// True when a pose `name` matches the filter `query`:
/// case-insensitive substring; an empty (or
/// whitespace-only) query matches everything
/// (figures_panel's `figure_name_matches`, same rule).
pub fn pose_name_matches(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

/// The REAL indices into `names` that the filter
/// `query` leaves visible, in list order. An empty
/// query yields every index. Rows are built from
/// these indices, so every row key and handler (apply,
/// mirror-apply, delete, rename) addresses the entry
/// it shows, filtered or not — figures_panel's
/// `visible_figures` discipline, carried over.
pub fn visible_pose_entries(names: &[String], query: &str) -> Vec<usize> {
    names
        .iter()
        .enumerate()
        .filter(|(_, name)| pose_name_matches(name, query))
        .map(|(i, _)| i)
        .collect()
}

/// The library heading's count line: with a filter
/// active, "{shown} of {total} poses"; with no
/// filter, the bare total (figures_panel's
/// `count_line` discipline).
pub fn pose_count_line(shown: usize, total: usize, query: &str) -> String {
    if pose_filter_active(query) {
        format!("{shown} of {total} poses")
    } else {
        format!("{total}")
    }
}

/// Save the selected figure's current-frame pose into the
/// pose library under the figure's display name. Not an
/// undo step — it writes a file, exactly as Copy pose (also
/// not an undo step) writes the clipboard. Saving the same
/// figure again overwrites its earlier entry.
pub fn save_current_pose(core: &mut EditorCore) {
    let Some(pose) = selected_pose(core).cloned() else {
        core.status = no_pose_status(core);
        return;
    };
    let name = figure_label(core, core.selected_figure);
    match save_pose_library(&pose, &name) {
        Ok(_) => {
            // The classifier's Success family is the
            // "Saved …" prefix — the shape the 3D pose
            // library's save already uses ("Saved 3D pose
            // '{name}'.", three_d_panel) and the shape a
            // classifier test pins for a library save.
            // "Pose saved …" missed the family and read as
            // a neutral Info (Wave 9 audit D7).
            core.status = format!("Saved pose '{name}'.");
        }
        Err(e) => {
            core.status = format!("Save pose failed: {e}");
        }
    }
}

/// Mirror a pose value: negate every angle, root
/// untouched. This is the exact mutation
/// `EditorCore::mirror_selected_pose` performs on the
/// current frame's pose (editor.rs:1450-1452), lifted
/// onto a value so it can be composed BEFORE a write:
/// the core op snapshots when it runs (editor.rs:1448),
/// so applying a pose and then calling the op would cost
/// two undo steps, while mirroring the loaded value and
/// writing once costs exactly one. The core op's stated
/// caveat carries over unchanged: per-node bends, segment
/// lengths and graph channels live outside `Pose` and are
/// not mirrored — the result is exact for the angle data
/// itself, and for the rendered figure on bilaterally
/// symmetric rigs (mirroring twice returns the original
/// exactly; the editor's own test pins the same
/// properties, editor.rs:5602).
pub fn mirror_pose(pose: &Pose) -> Pose {
    let mut mirrored = pose.clone();
    for a in &mut mirrored.angles {
        *a = -*a;
    }
    mirrored
}

/// Load the library pose `name` and write it over the
/// selected figure's current-frame pose, through the same
/// path Paste pose uses ([`write_current_pose`]: one
/// snapshot, one write — one undo step), with Paste's
/// guards: no target pose or an angle count that does not
/// match this figure's rig is a status-only refusal.
pub fn apply_library_pose(core: &mut EditorCore, name: &str) {
    apply_library_pose_in(core, &pose_library_dir(), name)
}

/// Load the library pose `name`, mirror the loaded value
/// (see [`mirror_pose`]), and write it over the selected
/// figure's current-frame pose — the same write path,
/// guards, and exactly-one-undo-step contract as a plain
/// apply.
pub fn apply_library_pose_mirrored(core: &mut EditorCore, name: &str) {
    apply_library_pose_mirrored_in(core, &pose_library_dir(), name)
}

fn apply_library_pose_in(core: &mut EditorCore, dir: &Path, name: &str) {
    apply_library_pose_in_mode(core, dir, name, false)
}

fn apply_library_pose_mirrored_in(core: &mut EditorCore, dir: &Path, name: &str) {
    apply_library_pose_in_mode(core, dir, name, true)
}

fn apply_library_pose_in_mode(
    core: &mut EditorCore,
    dir: &Path,
    name: &str,
    mirrored: bool,
) {
    let Some(cur) = selected_pose(core) else {
        core.status = no_pose_status(core);
        return;
    };
    let cur_len = cur.angles.len();
    let pose = match load_pose_library_in(dir, name) {
        Ok(p) => p,
        Err(e) => {
            core.status =
                format!("Apply pose failed: '{name}' could not be loaded ({e}).");
            return;
        }
    };
    if pose.angles.len() != cur_len {
        core.status = format!(
            "Apply pose failed: saved pose has {} angles, figure {} has {} segments.",
            pose.angles.len(),
            core.selected_figure + 1,
            cur_len
        );
        return;
    }
    let pose = if mirrored { mirror_pose(&pose) } else { pose };
    let (fi, fr) = (core.selected_figure + 1, core.current + 1);
    write_current_pose(core, pose);
    core.status = if mirrored {
        format!("Pose '{name}' applied mirrored to figure {fi} on frame {fr}.")
    } else {
        format!("Pose '{name}' applied to figure {fi} on frame {fr}.")
    };
}

/// Delete the library pose `name`'s file from the 2D
/// pose library (`./library/poses`). See
/// [`delete_library_pose_in`].
pub fn delete_library_pose(core: &mut EditorCore, name: &str) {
    delete_library_pose_in(core, &pose_library_dir(), name);
}

/// Delete the library pose `name`'s file from directory
/// `dir`. A filesystem operation, NOT an undo step — the
/// discipline Save established (a file operation is not
/// a project edit; there is nothing in the project to
/// restore). Returns whether the file was removed. On
/// failure the file is still there, so the scanned
/// listing is unchanged, and the error is reported. On
/// success the directory's mtime moves, which refreshes
/// the saved-name cache on the next build — the same
/// invalidation a Save relies on.
fn delete_library_pose_in(core: &mut EditorCore, dir: &Path, name: &str) -> bool {
    let path = pose_entry_file_in(dir, name);
    match std::fs::remove_file(&path) {
        Ok(()) => {
            core.status = format!("Pose '{name}' deleted from the library.");
            true
        }
        Err(e) => {
            core.status =
                format!("Delete pose failed: '{name}' could not be removed ({e}).");
            false
        }
    }
}

/// What renaming library pose `old` to the typed text
/// `raw_new` would do, decided purely from the scanned
/// entry names — the filesystem half lives in
/// [`rename_library_pose_in`].
#[derive(Clone, Debug, PartialEq)]
pub enum RenamePlan {
    /// The trimmed input is blank.
    Empty,
    /// The trimmed input IS the current name.
    SameName,
    /// Another entry already owns this name — or slugs
    /// to the same file, which for this library is the
    /// same collision (entries are addressed by slug).
    /// Carries the conflicting entry's name.
    Taken(String),
    /// Rename to the carried (trimmed) name.
    Ready(String),
}

/// The entry in `names` that blocks taking `new_name`,
/// if any: a DIFFERENT entry that either IS `new_name`
/// or slugs to the same file — two display names that
/// slug alike share one file, so the second would
/// silently overwrite the first's pose. The entry named
/// `except` (the one being renamed) never blocks itself.
fn name_conflict(names: &[String], new_name: &str, except: &str) -> Option<String> {
    let slug = pose_slug(new_name);
    names
        .iter()
        .find(|n| {
            n.as_str() != except && (n.as_str() == new_name || pose_slug(n) == slug)
        })
        .cloned()
}

/// True when a DIFFERENT entry in `names` blocks taking
/// `new_name` (see [`name_conflict`]).
pub fn pose_name_taken(names: &[String], new_name: &str, except: &str) -> bool {
    name_conflict(names, new_name, except).is_some()
}

/// Decide the [`RenamePlan`] for renaming `old` to the
/// typed text `raw_new`, against the scanned entry
/// `names` (which include `old`).
pub fn rename_plan(names: &[String], old: &str, raw_new: &str) -> RenamePlan {
    let new_name = raw_new.trim();
    if new_name.is_empty() {
        return RenamePlan::Empty;
    }
    if new_name == old {
        return RenamePlan::SameName;
    }
    if let Some(conflict) = name_conflict(names, new_name, old) {
        return RenamePlan::Taken(conflict);
    }
    RenamePlan::Ready(new_name.to_string())
}

/// Rename the library pose `old` in directory `dir` to
/// the display name typed by the user (`raw_new`).
///
/// The new entry file — `<slug(new)>.json` holding the
/// loaded pose under the new envelope name — is written
/// FIRST and the old file removed second, mirroring the
/// library panel's rename discipline (file first, then
/// the listing); here the listing is a directory scan,
/// so it follows the disk by itself on the next build
/// (the write/removal moves the directory mtime, which
/// also refreshes the name cache). Every refusal is
/// decided BEFORE any disk write ([`rename_plan`], plus
/// the existing-target-file guard below), so a refused
/// rename changes nothing on disk. A filesystem
/// operation, NOT an undo step, like Save and Delete.
///
/// Returns whether the rename session is settled:
/// `true` when the editor should close (renamed, the
/// trimmed name already IS `old`'s, or `old` no longer
/// exists in the scan); `false` when the user should
/// keep editing, with the reason on `core.status`.
fn rename_library_pose_in(
    core: &mut EditorCore,
    dir: &Path,
    old: &str,
    raw_new: &str,
) -> bool {
    let names = scan_pose_library_in(dir);
    if !names.iter().any(|n| n.as_str() == old) {
        // The entry vanished (deleted since the session
        // opened): nothing left to rename or settle
        // against.
        return true;
    }
    let new_name = match rename_plan(&names, old, raw_new) {
        RenamePlan::Empty => {
            core.status = "Type a new name for the pose first.".to_string();
            return false;
        }
        RenamePlan::SameName => {
            core.status = format!("Pose '{old}' already has that name.");
            return true;
        }
        RenamePlan::Taken(conflict) => {
            core.status =
                format!("A pose named '{conflict}' already uses that name.");
            return false;
        }
        RenamePlan::Ready(name) => name,
    };
    let old_path = pose_entry_file_in(dir, old);
    let new_path = dir.join(pose_file_name(&new_name));
    if new_path != old_path && new_path.exists() {
        // A file the scan could not read as a pose entry
        // (e.g. corrupt JSON) still owns that slug —
        // writing over it would silently destroy it
        // (the library panel's rename refuses the same
        // case).
        core.status = format!(
            "A file named '{}' already exists in the pose library folder.",
            pose_file_name(&new_name)
        );
        return false;
    }
    let pose = match load_pose_library_in(dir, old) {
        Ok(p) => p,
        Err(e) => {
            core.status =
                format!("Rename pose failed: '{old}' could not be loaded ({e}).");
            return false;
        }
    };
    if let Err(e) = save_pose_library_in(dir, &pose, &new_name) {
        core.status = format!("Rename pose failed: {e}");
        return false;
    }
    if new_path != old_path {
        if let Err(e) = std::fs::remove_file(&old_path) {
            // The rename itself landed — the new file is
            // written and loads under the new name — but
            // the stale old file remains and will list as
            // a second entry until it is deleted. Say
            // exactly that; the session is settled. The
            // wording uses "cannot" deliberately: it puts
            // the partial failure in the status
            // classifier's warning grammar instead of
            // letting it read as a plain confirmation.
            core.status = format!(
                "Pose '{old}' renamed to '{new_name}', but the old file cannot be removed ({e})."
            );
            return true;
        }
    }
    core.status = format!("Pose '{old}' renamed to '{new_name}'.");
    true
}

/// Rename-session state for the pose library: which
/// listed entry is being renamed and the name being
/// typed. Unlike this panel's name cache this cannot
/// live in a thread-local: a `rune::TextEdit` is not
/// `Copy`, and `text_input`'s `on_edit` needs a
/// `&mut TextEdit` path into the shell `State` (the
/// wall clips_panel's rename hit — its module docs tell
/// the same story). The integrator homes this in
/// `State` beside `pose_clipboard`; the exact lines are
/// in this file's module docs.
pub struct PoseRenameState {
    /// Index into the sorted [`saved_pose_names`] list
    /// of the entry being renamed, or `None` while no
    /// rename is open. By the time Apply runs this is
    /// only the open/closed flag — the entry itself is
    /// resolved through [`PoseRenameState::name`], so a
    /// listing that shifted mid-session (a delete above
    /// the target) cannot rename the wrong pose.
    pub target: Option<usize>,
    /// The display name captured when the session
    /// opened — the rename's source of truth.
    pub name: String,
    /// The in-progress name, loaded from the entry by
    /// [`start_rename`].
    pub field: TextEdit,
}

impl PoseRenameState {
    pub fn new() -> Self {
        PoseRenameState {
            target: None,
            name: String::new(),
            field: TextEdit::new(""),
        }
    }
}

/// Open the rename editor for saved-pose row `i` (the
/// per-row Rename button's handler): load the entry's
/// current name into the field and mark the row as
/// being renamed. The fresh `TextEdit`'s `focused` flag
/// claims keyboard focus on the next commit, so the
/// integrator's `text_focused` must include the field —
/// see the module docs. Out-of-range rows open nothing.
pub fn start_rename(s: &mut State, i: usize) {
    let saved = saved_pose_names();
    let Some(name) = saved.get(i).cloned() else {
        return;
    };
    s.pose_rename.field = TextEdit::new(name.clone());
    s.pose_rename.field.focused = true;
    s.pose_rename.target = Some(i);
    s.pose_rename.name = name;
}

/// Apply the open rename (the Apply button's handler
/// and the field's submit handler): run the rename with
/// the field's text, and close the session only when it
/// settles — a refused rename stays open, status line
/// explaining why, so the name can be corrected.
pub fn apply_rename(s: &mut State) {
    if s.pose_rename.target.is_none() {
        return;
    }
    let old = s.pose_rename.name.clone();
    let raw = s.pose_rename.field.text.clone();
    if rename_library_pose_in(&mut s.core, &pose_library_dir(), &old, &raw) {
        s.pose_rename.target = None;
        s.pose_rename.name.clear();
        s.pose_rename.field = TextEdit::default();
    }
}

/// Abandon the open rename (the Cancel button's
/// handler): close the session and drop the typed text.
/// Nothing on disk was touched — renames only happen in
/// [`apply_rename`]. Also used to settle the session of
/// an entry that was just deleted.
pub fn cancel_rename(s: &mut State) {
    s.pose_rename.target = None;
    s.pose_rename.name.clear();
    s.pose_rename.field = TextEdit::default();
}

/// The rename editor row rendered under the library row
/// being renamed: the name field plus Apply and Cancel.
/// Only one rename session can be open, so the keys are
/// unindexed — the row's identity comes from its
/// position under the renamed entry (the library
/// panel's convention).
fn rename_editor_row(s: &State, t: &Theme) -> El {
    row()
        .key("pose-rename-row")
        .gap(4.0)
        .center_y()
        .child(
            design::text_field(&s.pose_rename.field, type_::BODY.lh(26.0), s.pose_rename.field.focused, t)
                .key("pose-rename")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder("New name", t.text_secondary)
                .on_edit(|s: &mut State| &mut s.pose_rename.field)
                .on_submit(|s: &mut State, _tx: &mut Tx| apply_rename(s)),
        )
        .child(icon_btn(
            "pose-rename-apply",
            "check",
            "Apply",
            false,
            t,
            |s| apply_rename(s),
        ))
        .child(icon_btn(
            "pose-rename-cancel",
            "xmark",
            "Cancel",
            false,
            t,
            |s| cancel_rename(s),
        ))
}

/// The glyph on the library Apply-mirrored buttons:
/// Font Awesome's two-way horizontal arrows — the
/// honest flip glyph, in the catalogue since Phase I
/// section 10. Phase H shipped `copy` here because no
/// flip glyph existed; this constant exists so the
/// choice is pinned by a test rather than buried in a
/// call site.
pub const APPLY_MIRRORED_ICON: &str = "arrow-right-arrow-left";

/// The Pose tools section: a labelled group with the pose
/// ops for the selected figure on the current frame.
pub fn pose_tools(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let has_pose = has_selected_pose(core);
    let can_paste = s.pose_clipboard.is_some() && has_pose;
    let caption = if core.selected_figure < core.project.figures.len() {
        figure_label(core, core.selected_figure)
    } else {
        "No figure".to_string()
    };
    let mut body = design::panel_col("pose-tools", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Pose tools", Some(&caption), t)),
        )
        .child(design::divider(t))
        .child(
            row()
                .gap(4.0)
                .child(if has_pose {
                    btn("pose-mirror", "Mirror pose", false, t, |s| {
                        s.core.mirror_selected_pose()
                    })
                } else {
                    btn_inert("pose-mirror", "Mirror pose", t)
                })
                .child(if has_pose {
                    btn(
                        "pose-mirror-all",
                        "Mirror figure (all frames)",
                        false,
                        t,
                        |s| {
                            s.core
                                .mirror_figure_all_frames(s.core.selected_figure);
                        },
                    )
                } else {
                    btn_inert("pose-mirror-all", "Mirror figure (all frames)", t)
                }),
        )
        .child(
            row()
                .gap(4.0)
                .flex_wrap()
                .child(if has_pose {
                    icon_btn("pose-copy", "copy", "Copy pose", false, t, |s| {
                        copy_pose(&mut s.core, &mut s.pose_clipboard)
                    })
                } else {
                    icon_btn_inert("pose-copy", "copy", "Copy pose", t)
                })
                .child(if can_paste {
                    icon_btn("pose-paste", "clipboard", "Paste pose", false, t, |s| {
                        paste_pose(&mut s.core, &s.pose_clipboard)
                    })
                } else {
                    icon_btn_inert("pose-paste", "clipboard", "Paste pose", t)
                })
                .child(if can_paste {
                    icon_btn(
                        "pose-apply-all",
                        "check",
                        "Apply pose to all frames",
                        false,
                        t,
                        |s| apply_pose_to_all_frames(&mut s.core, &s.pose_clipboard),
                    )
                } else {
                    icon_btn_inert(
                        "pose-apply-all",
                        "check",
                        "Apply pose to all frames",
                        t,
                    )
                }),
        )
        .child(
            row()
                .gap(4.0)
                .child(if has_pose {
                    btn("pose-reset", "Reset pose", false, t, |s| {
                        reset_pose(&mut s.core)
                    })
                } else {
                    btn_inert("pose-reset", "Reset pose", t)
                })
                .child(if can_bake(core) {
                    btn("pose-bake-graph", "Bake graph", false, t, |s| {
                        let last = s.core.frame_count().saturating_sub(1);
                        s.core.bake_graph_channels(0, last);
                    })
                } else {
                    btn_inert("pose-bake-graph", "Bake graph", t)
                }),
        )
        .child(row().gap(4.0).child(if has_pose {
            icon_btn("pose-save-lib", "floppy-disk", "Save pose", false, t, |s| {
                save_current_pose(&mut s.core)
            })
        } else {
            icon_btn_inert("pose-save-lib", "floppy-disk", "Save pose", t)
        }));
    let saved = saved_pose_names();
    let query = s.pose_library.filter.text.clone();
    let visible = visible_pose_entries(&saved, &query);
    // The section header renders ALWAYS (UI mission
    // Wave 4 batch 2, audit S07-12a): the whole
    // section used to vanish with an empty library,
    // so a new user never learned the library exists
    // or that Save pose (above) is what populates it.
    body = body.child(
        row()
            .key("pose-lib-header")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Pose library", t))
            .child(
                text(pose_count_line(visible.len(), saved.len(), &query))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
    );
    if saved.is_empty() {
        body = body.child(design::empty_state(
            "shapes",
            "No saved poses yet — Save pose stores the current pose here.",
            t,
        ));
    } else {
        body = body
            // Filter field (Phase I), mirroring the
            // figures filter: typing is the only writer,
            // and the list below filters on the text
            // every render. View-only — no engine op.
            .child(
                row()
                    .key("pose-lib-filter-row")
                    .gap(8.0)
                    .center_y()
                    .child(icon_el("magnifying-glass", 13.0, t.text_secondary))
                    .child(
                        design::text_field(&s.pose_library.filter, type_::BODY.lh(26.0), s.pose_library.filter.focused, t)
                            .key("pose-lib-filter")
                            .single_line()
                            .grow(1.0)
                            .color(t.text)
                            .placeholder("Filter poses", t.text_secondary)
                            .on_edit(|s: &mut State| &mut s.pose_library.filter),
                    ),
            );
        for &i in &visible {
            let name = &saved[i];
            let apply_name = name.clone();
            let mirror_name = name.clone();
            let del_name = name.clone();
            // The entry being renamed carries the kit
            // selected fill — the row state the rename
            // editor below it belongs to (the library
            // panel's convention).
            let renaming_this = s.pose_rename.target.is_some()
                && s.pose_rename.name.as_str() == name.as_str();
            // Two-line variant of the kit list-row
            // anatomy (P8): the same px, radius, selected
            // fill and hover as `design::list_row`,
            // applied to a `col` because the entry
            // carries two control lines (the kit builder
            // itself is a single-line `row`).
            let entry_root = col()
                .key(format!("pose-lib-row-{i}"))
                .w_full()
                .gap(4.0)
                .px(8.0)
                .py(4.0)
                .radius(design::ROW_RADIUS)
                .min_h(28.0);
            let entry_root = if renaming_this {
                entry_root.fill(design::selected_fill())
            } else {
                entry_root.hover(t.hover)
            };
            let mut entry = entry_root
                .child(
                    row()
                        .gap(4.0)
                        .center_y()
                        // The name is an unbounded user string
                        // (Save takes the figure's name, Rename
                        // accepts anything typed): without
                        // Rune's one-line truncate mode it would
                        // measure at full width and push the
                        // Rename / Del buttons past the card
                        // edge — the figures_panel row name
                        // truncates for exactly this reason.
                        .child(
                            text(name.as_str())
                                .font(type_::LABEL)
                                .color(t.text)
                                .truncate(),
                        )
                        .child(el().grow(1.0))
                        .child(icon_btn(
                            &format!("pose-lib-rename-{i}"),
                            "pencil",
                            &i18n_wire::tr(core, "rename"),
                            false,
                            t,
                            move |s: &mut State| start_rename(s, i),
                        ))
                        .child(danger_icon_btn(
                            &format!("pose-lib-del-{i}"),
                            "trash-can",
                            "Del",
                            false,
                            t,
                            move |s: &mut State| {
                                // Deleting the entry being
                                // renamed settles its
                                // session first (the
                                // library panel's
                                // discipline).
                                if s.pose_rename.name == del_name {
                                    cancel_rename(s);
                                }
                                delete_library_pose(&mut s.core, &del_name);
                            },
                        )),
                )
                .child(
                    row()
                        .gap(4.0)
                        .child(if has_pose {
                            icon_btn(
                                &format!("pose-lib-apply-{i}"),
                                "check",
                                "Apply",
                                false,
                                t,
                                move |s: &mut State| {
                                    apply_library_pose(&mut s.core, &apply_name)
                                },
                            )
                        } else {
                            icon_btn_inert(
                                &format!("pose-lib-apply-{i}"),
                                "check",
                                "Apply",
                                t,
                            )
                        })
                        .child(if has_pose {
                            icon_btn(
                                &format!("pose-lib-apply-mirror-{i}"),
                                APPLY_MIRRORED_ICON,
                                "Apply mirrored",
                                false,
                                t,
                                move |s: &mut State| {
                                    apply_library_pose_mirrored(
                                        &mut s.core,
                                        &mirror_name,
                                    )
                                },
                            )
                        } else {
                            icon_btn_inert(
                                &format!("pose-lib-apply-mirror-{i}"),
                                APPLY_MIRRORED_ICON,
                                "Apply mirrored",
                                t,
                            )
                        }),
                );
            if renaming_this {
                entry = entry.child(rename_editor_row(s, t));
            }
            body = body.child(entry);
        }
        if visible.is_empty() {
            body = body.child(design::empty_state(
                "magnifying-glass",
                &format!("No poses match \"{}\".", query.trim()),
                t,
            ));
        }
    }
    let targets = pose_targets(core);
    if !targets.is_empty() {
        body = body.child(design::section_label("Copy pose to", t));
        let mut trow = row().gap(4.0);
        for j in targets {
            let enabled = has_pose && pose_exists(core, j);
            let key = format!("pose-copy-to-{}", j + 1);
            let label = figure_label(core, j);
            trow = trow.child(if enabled {
                btn(&key, &label, false, t, move |s: &mut State| {
                    copy_pose_to(&mut s.core, j)
                })
            } else {
                btn_inert(&key, &label, t)
            });
        }
        body = body.child(trow.scroll_x().w_full());
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pose_tools_i18n_keys_exist_and_match_the_english_labels() {
        // Wave V (V11, from U10's catalogue-hygiene
        // ledger): the pose-library row's Rename button
        // resolves through i18n_wire::tr under the
        // pre-existing catalogue key (one key serves all
        // four Rename buttons). The key already existed
        // x28, so this pin passes on its own merits — no
        // IA was owed.
        let core = EditorCore::new();
        for (key, english) in [("rename", "Rename")] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

    fn core() -> EditorCore {
        EditorCore::new()
    }

    #[test]
    fn demo_core_has_a_selected_pose_and_no_targets() {
        let c = core();
        assert!(has_selected_pose(&c));
        assert!(selected_pose(&c).is_some());
        // The demo project has one figure, so there is
        // nothing to copy a pose *to* yet.
        assert!(pose_targets(&c).is_empty());
    }

    #[test]
    fn bake_is_unavailable_without_channels() {
        let mut c = core();
        c.project.channels.clear();
        assert!(!can_bake(&c));
    }

    #[test]
    fn copy_then_paste_moves_the_pose_to_another_frame() {
        let mut c = core();
        let mut clip = None;
        // Make frame 0's pose distinctive, then copy it.
        c.mirror_selected_pose();
        let mirrored = selected_pose(&c).unwrap().clone();
        copy_pose(&mut c, &mut clip);
        assert!(clip.is_some());
        let last = c.frame_count() - 1;
        assert!(last > 0, "demo project needs at least two frames");
        c.goto(last);
        paste_pose(&mut c, &clip);
        let pasted = selected_pose(&c).unwrap();
        assert_eq!(pasted.angles, mirrored.angles);
        assert_eq!(pasted.root, mirrored.root);
    }

    #[test]
    fn paste_without_copy_is_a_status_only_noop() {
        let mut c = core();
        let before = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &None);
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Nothing copied"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn paste_is_exactly_one_undo_step() {
        let mut c = core();
        let mut clip = None;
        c.mirror_selected_pose();
        copy_pose(&mut c, &mut clip);
        c.mirror_selected_pose(); // back to the original pose
        let orig = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &clip);
        assert_ne!(selected_pose(&c).unwrap().angles, orig.angles);
        c.undo();
        assert_eq!(selected_pose(&c).unwrap().angles, orig.angles);
    }

    #[test]
    fn reset_pose_restores_rest_angles_and_keeps_root() {
        let mut c = core();
        c.mirror_selected_pose();
        let root_before = selected_pose(&c).unwrap().root;
        reset_pose(&mut c);
        let p = selected_pose(&c).unwrap();
        let rest = c.project.figures[c.selected_figure].def.rest_pose(p.root);
        assert_eq!(p.angles, rest.angles);
        assert_eq!(p.root, root_before);
    }

    #[test]
    fn targets_exclude_the_selected_figure() {
        let mut c = core();
        c.add_figure();
        c.add_figure();
        // add_figure selects the figure it just added.
        assert_eq!(c.selected_figure, 2);
        assert_eq!(pose_targets(&c), vec![0, 1]);
    }

    #[test]
    fn paste_refuses_a_mismatched_rig() {
        let mut c = core();
        let mut short = selected_pose(&c).unwrap().clone();
        short.angles.clear();
        let clip = Some(short);
        let before = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &clip);
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Paste pose failed"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn copy_pose_to_copies_between_figures_as_one_undo_step() {
        let mut c = core();
        // A distinctive source pose on figure 1.
        c.mirror_selected_pose();
        let mirrored = selected_pose(&c).unwrap().clone();
        c.add_figure();
        // add_figure selects the figure it just added;
        // the copy goes FROM figure 1 TO figure 2.
        c.selected_figure = 0;
        let depth = c.undo_depth();
        copy_pose_to(&mut c, 1);
        assert_eq!(c.undo_depth(), depth + 1);
        let target = &c.project.frames[c.current].poses[1];
        assert_eq!(target.angles, mirrored.angles);
        assert_eq!(target.root, mirrored.root);
        assert!(
            c.status.contains("Copied figure 1's pose to figure 2"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn copy_pose_to_refuses_a_mismatched_rig() {
        // Wave 10 audit D7: the engine op has no rig
        // check, so the panel-side guard is the only
        // thing between a differently-rigged target and
        // a pose row `model::evaluate` would panic on.
        // The target's row losing its angles is the
        // paste tests' mismatch idiom.
        let mut c = core();
        c.add_figure();
        c.selected_figure = 0;
        c.project.frames[c.current].poses[1].angles.clear();
        let before = c.project.frames[c.current].poses[1].clone();
        let depth = c.undo_depth();
        copy_pose_to(&mut c, 1);
        assert_eq!(c.undo_depth(), depth, "a refused copy takes no snapshot");
        assert_eq!(
            c.project.frames[c.current].poses[1].angles,
            before.angles,
            "the target row is untouched"
        );
        assert!(
            c.status.contains("Copy pose failed"),
            "unexpected status: {}",
            c.status
        );
    }

    // ---- Pose library (Phase F) ----

    /// A fresh per-test library directory under the OS temp
    /// dir (fig3d's test pattern), removed on entry so a
    /// previous run's leftovers cannot leak in.
    fn temp_library(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykr-pose-tools-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn pose_file_stem_slugs_like_the_3d_library() {
        assert_eq!(pose_file_stem("Styk"), "styk");
        assert_eq!(pose_file_stem("Figure 1"), "figure-1");
        assert_eq!(pose_file_stem("  Wave Pose!! "), "wave-pose");
        assert_eq!(pose_file_stem("BOSS Fight"), "boss-fight");
        // Nothing usable falls back to "figure", exactly as
        // fig3d::library_slug does.
        assert_eq!(pose_file_stem(""), "figure");
        assert_eq!(pose_file_stem("!!!"), "figure");
    }

    #[test]
    fn pose_serialise_round_trips_through_the_library() {
        let dir = temp_library("roundtrip");
        let c = core();
        let pose = selected_pose(&c).unwrap().clone();
        // A directory that does not exist yet scans empty,
        // and loading a name that was never saved is an
        // error (fig3d's round-trip test pins the same).
        assert!(scan_pose_library_in(&dir).is_empty());
        assert!(load_pose_library_in(&dir, "Styk").is_err());
        let path = save_pose_library_in(&dir, &pose, "Styk").unwrap();
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "styk.json"
        );
        assert_eq!(scan_pose_library_in(&dir), vec!["Styk".to_string()]);
        let back = load_pose_library_in(&dir, "Styk").unwrap();
        assert_eq!(back.angles, pose.angles);
        assert_eq!(back.root, pose.root);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_the_same_name_twice_overwrites() {
        let dir = temp_library("overwrite");
        let mut c = core();
        let first = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &first, "Styk").unwrap();
        c.mirror_selected_pose();
        let second = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &second, "Styk").unwrap();
        // One slot per name: the list still has one entry
        // and it holds the newer pose.
        assert_eq!(scan_pose_library_in(&dir).len(), 1);
        let back = load_pose_library_in(&dir, "Styk").unwrap();
        assert_eq!(back.angles, second.angles);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_library_pose_is_exactly_one_undo_step() {
        let dir = temp_library("apply");
        let mut c = core();
        c.mirror_selected_pose();
        let mirrored = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &mirrored, "Styk").unwrap();
        c.mirror_selected_pose(); // back to the original pose
        let orig = selected_pose(&c).unwrap().clone();
        apply_library_pose_in(&mut c, &dir, "Styk");
        assert_eq!(selected_pose(&c).unwrap().angles, mirrored.angles);
        assert!(
            c.status.contains("applied to figure"),
            "unexpected status: {}",
            c.status
        );
        c.undo();
        assert_eq!(selected_pose(&c).unwrap().angles, orig.angles);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_library_pose_keeps_states_limits_and_out_of_limit_angles() {
        // Wave Z (Z9, library state round-trip audit): the
        // pose library stores a Pose only, so applying one
        // writes ONLY the target frame's pose row — the
        // figure's animation states (Wave X, P-1, on the
        // instance) and its joint limits (P-4, on the def)
        // are not the library's data and must survive an
        // apply untouched, and the applied angles land
        // verbatim even past a limit (limits bind solving,
        // never stored poses — P-4's standing rule).
        let dir = temp_library("z9-apply");
        let mut c = core();
        let states = vec![stykanimate_rs::model::StateKey {
            frame: 1,
            clip: 0,
            offset: 2,
            speed: 1.5,
        }];
        c.project.figures[0].states = states.clone();
        let mut pose = selected_pose(&c).unwrap().clone();
        pose.angles[4] = 170.0; // past the elbow preset max (+160)
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        apply_library_pose_in(&mut c, &dir, "Styk");
        assert!(
            c.status.contains("applied to figure"),
            "unexpected status: {}",
            c.status
        );
        assert_eq!(
            selected_pose(&c).unwrap().angles,
            pose.angles,
            "the over-limit angle lands verbatim, unclamped"
        );
        assert_eq!(
            c.project.figures[0].states, states,
            "the figure's state keys survive a pose apply"
        );
        assert_eq!(c.project.figures[0].def.nodes[4].min_angle, Some(-10.0));
        assert_eq!(c.project.figures[0].def.nodes[4].max_angle, Some(160.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_library_pose_refuses_a_mismatched_rig() {
        let dir = temp_library("mismatch");
        let mut c = core();
        let mut short = selected_pose(&c).unwrap().clone();
        short.angles.clear();
        save_pose_library_in(&dir, &short, "Short").unwrap();
        let before = selected_pose(&c).unwrap().clone();
        apply_library_pose_in(&mut c, &dir, "Short");
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Apply pose failed"),
            "unexpected status: {}",
            c.status
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_library_pose_to_a_same_count_figure_keeps_its_own_lengths() {
        // Wave 10 audit D7, area 1 pin: a Pose is root +
        // angles only — segment lengths live in the
        // figure's own def — so applying a saved pose to
        // a figure with the same angle count but
        // different proportions transfers the angles
        // exactly and leaves the target's lengths (its
        // proportions) untouched: no silent distortion.
        // The different-count case is the refusal pinned
        // by the test above.
        let dir = temp_library("proportions");
        let mut c = core();
        c.mirror_selected_pose();
        let saved = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &saved, "Styk").unwrap();
        c.add_figure();
        // add_figure selects the figure it added; give
        // it visibly different proportions.
        let longer = c.project.figures[1].def.nodes[1].length * 2.0;
        c.project.figures[1].def.nodes[1].length = longer;
        apply_library_pose_in(&mut c, &dir, "Styk");
        let got = selected_pose(&c).unwrap();
        assert_eq!(got.angles, saved.angles);
        assert_eq!(got.root, saved.root);
        assert_eq!(c.project.figures[1].def.nodes[1].length, longer);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cached_pose_names_serves_until_the_key_changes() {
        let mut cache: PoseNameCache = None;
        let reads = std::cell::Cell::new(0usize);
        let read = || {
            reads.set(reads.get() + 1);
            vec!["A".to_string()]
        };
        let key1 = Some(SystemTime::UNIX_EPOCH);
        let key2 = Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1));
        assert_eq!(
            cached_pose_names(&mut cache, key1, read),
            vec!["A".to_string()]
        );
        // Same key: served from the slot, no second read.
        assert_eq!(
            cached_pose_names(&mut cache, key1, read),
            vec!["A".to_string()]
        );
        assert_eq!(reads.get(), 1);
        // A changed key (the directory was touched) re-reads.
        assert_eq!(
            cached_pose_names(&mut cache, key2, read),
            vec!["A".to_string()]
        );
        assert_eq!(reads.get(), 2);
    }

    // ---- Pose library Phase G: mirror / delete / rename ----

    #[test]
    fn mirror_pose_negates_every_angle_and_keeps_the_root() {
        // Expectations derived from the engine op's own
        // test (editor.rs mirror_pose_negates_angles_and_
        // is_its_own_inverse): root unmoved, every angle
        // negated, twice = the exact original.
        let pose = Pose {
            root: [12.0, -4.0],
            angles: vec![10.0, -25.0, 45.0, 130.5],
        };
        let mirrored = mirror_pose(&pose);
        assert_eq!(mirrored.root, pose.root);
        assert_eq!(mirrored.angles, vec![-10.0, 25.0, -45.0, -130.5]);
        assert_eq!(mirror_pose(&mirrored).angles, pose.angles);
        assert_eq!(mirror_pose(&mirrored).root, pose.root);
    }

    #[test]
    fn mirror_apply_matches_the_engine_mirror_op() {
        let dir = temp_library("mirror-apply");
        let saver = core();
        let plain = selected_pose(&saver).unwrap().clone();
        save_pose_library_in(&dir, &plain, "Styk").unwrap();
        // The engine op's result on a fresh core...
        let mut expected = core();
        expected.mirror_selected_pose();
        let expected = selected_pose(&expected).unwrap().clone();
        // ...is exactly what mirror-apply produces.
        let mut c = core();
        apply_library_pose_mirrored_in(&mut c, &dir, "Styk");
        let got = selected_pose(&c).unwrap();
        assert_eq!(got.angles, expected.angles);
        assert_eq!(got.root, expected.root);
        assert!(
            c.status.contains("applied mirrored"),
            "unexpected status: {}",
            c.status
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mirror_apply_is_exactly_one_undo_step() {
        let dir = temp_library("mirror-undo");
        let mut c = core();
        let orig = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &orig, "Styk").unwrap();
        apply_library_pose_mirrored_in(&mut c, &dir, "Styk");
        // The demo pose is asymmetric (the paste undo
        // test relies on the same fact), so the mirrored
        // apply visibly changed it...
        assert_ne!(selected_pose(&c).unwrap().angles, orig.angles);
        // ...and ONE undo restores it — no second hidden
        // snapshot from a composed mirror op.
        c.undo();
        assert_eq!(selected_pose(&c).unwrap().angles, orig.angles);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mirror_apply_refuses_a_mismatched_rig() {
        let dir = temp_library("mirror-mismatch");
        let mut c = core();
        let mut short = selected_pose(&c).unwrap().clone();
        short.angles.clear();
        save_pose_library_in(&dir, &short, "Short").unwrap();
        let before = selected_pose(&c).unwrap().clone();
        apply_library_pose_mirrored_in(&mut c, &dir, "Short");
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Apply pose failed"),
            "unexpected status: {}",
            c.status
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_library_pose_removes_the_file_and_the_listing() {
        let dir = temp_library("delete");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        save_pose_library_in(&dir, &pose, "Wave").unwrap();
        assert!(delete_library_pose_in(&mut c, &dir, "Styk"));
        assert!(
            c.status.contains("deleted"),
            "unexpected status: {}",
            c.status
        );
        assert_eq!(scan_pose_library_in(&dir), vec!["Wave".to_string()]);
        assert!(!dir.join("styk.json").exists());
        assert!(dir.join("wave.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_missing_pose_reports_and_changes_nothing() {
        let dir = temp_library("delete-missing");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        // remove_file on a name that was never saved is
        // the failure path: status reports it and the
        // listing (a fresh scan) is untouched.
        assert!(!delete_library_pose_in(&mut c, &dir, "Ghost"));
        assert!(
            c.status.contains("Delete pose failed"),
            "unexpected status: {}",
            c.status
        );
        assert_eq!(scan_pose_library_in(&dir), vec!["Styk".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_plan_decides_every_branch() {
        let names = vec!["Styk".to_string(), "Wave Pose".to_string()];
        assert_eq!(rename_plan(&names, "Styk", "   "), RenamePlan::Empty);
        assert_eq!(
            rename_plan(&names, "Styk", " Styk "),
            RenamePlan::SameName
        );
        // Exact-name collision with another entry.
        assert_eq!(
            rename_plan(&names, "Styk", "Wave Pose"),
            RenamePlan::Taken("Wave Pose".to_string())
        );
        // Slug collision: "wave pose" is a different
        // display name but the SAME file as "Wave Pose".
        assert_eq!(
            rename_plan(&names, "Styk", "wave pose"),
            RenamePlan::Taken("Wave Pose".to_string())
        );
        // The entry's own slug is not a collision.
        assert_eq!(
            rename_plan(&names, "Wave Pose", "wave pose"),
            RenamePlan::Ready("wave pose".to_string())
        );
        // Ready carries the trimmed name.
        assert_eq!(
            rename_plan(&names, "Styk", "  Hero Pose "),
            RenamePlan::Ready("Hero Pose".to_string())
        );
        assert!(pose_name_taken(&names, "Wave Pose", "Styk"));
        assert!(!pose_name_taken(&names, "Wave Pose", "Wave Pose"));
        assert!(!pose_name_taken(&names, "Hero", "Styk"));
    }

    #[test]
    fn rename_moves_the_file_and_updates_the_envelope() {
        let dir = temp_library("rename");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        assert!(rename_library_pose_in(&mut c, &dir, "Styk", "Hero Pose"));
        assert!(
            c.status.contains("renamed to"),
            "unexpected status: {}",
            c.status
        );
        // The scan reads envelope names, so this also
        // proves the new file's envelope was updated.
        assert_eq!(
            scan_pose_library_in(&dir),
            vec!["Hero Pose".to_string()]
        );
        assert!(dir.join("hero-pose.json").exists());
        assert!(!dir.join("styk.json").exists());
        let back = load_pose_library_in(&dir, "Hero Pose").unwrap();
        assert_eq!(back.angles, pose.angles);
        assert_eq!(back.root, pose.root);
        assert!(load_pose_library_in(&dir, "Styk").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_refusals_touch_nothing() {
        let dir = temp_library("rename-refuse");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        save_pose_library_in(&dir, &pose, "Wave Pose").unwrap();
        // Blank input keeps the session open.
        assert!(!rename_library_pose_in(&mut c, &dir, "Styk", "  "));
        assert!(
            c.status.contains("Type a new name"),
            "unexpected status: {}",
            c.status
        );
        // Taken, exactly and by slug.
        assert!(!rename_library_pose_in(&mut c, &dir, "Styk", "Wave Pose"));
        assert!(!rename_library_pose_in(&mut c, &dir, "Styk", "wave pose"));
        // Same name settles (session closes) as a no-op.
        assert!(rename_library_pose_in(&mut c, &dir, "Styk", "Styk"));
        assert!(
            c.status.contains("already has that name"),
            "unexpected status: {}",
            c.status
        );
        // Nothing moved through any of it.
        assert_eq!(
            scan_pose_library_in(&dir),
            vec!["Styk".to_string(), "Wave Pose".to_string()]
        );
        assert_eq!(
            load_pose_library_in(&dir, "Styk").unwrap().angles,
            pose.angles
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hand_placed_entry_loads_renames_and_deletes_by_its_stored_name() {
        // Wave W, W9: the listing reads names out of file
        // contents, so an entry whose file stem is not its
        // stored name's slug is listed — resolution must be
        // the scan's inverse, or the entry could be seen
        // but never loaded, renamed, or deleted.
        let dir = temp_library("hand-placed");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        let canonical = save_pose_library_in(&dir, &pose, "Wave Pose").unwrap();
        let placed = dir.join("backup.json");
        std::fs::rename(&canonical, &placed).unwrap();
        assert_eq!(
            scan_pose_library_in(&dir),
            vec!["Wave Pose".to_string()]
        );
        assert_eq!(pose_entry_file_in(&dir, "Wave Pose"), placed);
        assert_eq!(
            load_pose_library_in(&dir, "Wave Pose").unwrap().angles,
            pose.angles
        );
        // Rename moves the hand-placed file, not the
        // (absent) slug file.
        assert!(rename_library_pose_in(&mut c, &dir, "Wave Pose", "Hero Pose"));
        assert!(!placed.exists());
        assert!(dir.join("hero-pose.json").exists());
        assert_eq!(
            scan_pose_library_in(&dir),
            vec!["Hero Pose".to_string()]
        );
        // Delete removes the file the entry actually lives
        // in, even after another external rename.
        let placed2 = dir.join("odd name.json");
        std::fs::rename(dir.join("hero-pose.json"), &placed2).unwrap();
        assert!(delete_library_pose_in(&mut c, &dir, "Hero Pose"));
        assert!(!placed2.exists());
        assert!(scan_pose_library_in(&dir).is_empty());
        // A name no file stores falls back to the slug path.
        assert_eq!(
            pose_entry_file_in(&dir, "Ghost"),
            dir.join("ghost.json")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_refuses_an_unreadable_target_file() {
        let dir = temp_library("rename-target");
        let mut c = core();
        let pose = selected_pose(&c).unwrap().clone();
        save_pose_library_in(&dir, &pose, "Styk").unwrap();
        // A corrupt file already owns the "hero" slug;
        // the scan skips it (it is not a pose entry), but
        // the rename must not silently overwrite it.
        std::fs::write(dir.join("hero.json"), "not a pose").unwrap();
        assert!(!rename_library_pose_in(&mut c, &dir, "Styk", "Hero"));
        assert!(
            c.status.contains("already exists"),
            "unexpected status: {}",
            c.status
        );
        assert_eq!(scan_pose_library_in(&dir), vec!["Styk".to_string()]);
        assert!(dir.join("styk.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Pose library Phase I: filter + mirror glyph ----

    #[test]
    fn library_state_default_filter_is_empty() {
        let st = PoseLibraryState::new();
        assert!(st.filter.text.is_empty());
        assert!(!st.filter.focused);
    }

    #[test]
    fn pose_name_matches_is_case_insensitive_substring() {
        assert!(pose_name_matches("Wave Pose", "wave"));
        assert!(pose_name_matches("Wave Pose", "WAVE"));
        assert!(pose_name_matches("Wave Pose", "ve Po"));
        assert!(pose_name_matches("Styk", "STY"));
        assert!(!pose_name_matches("Styk", "wave"));
        assert!(!pose_name_matches("Styk", "stykk"));
    }

    #[test]
    fn pose_name_matches_empty_query_matches_everything() {
        assert!(pose_name_matches("Styk", ""));
        assert!(pose_name_matches("Styk", "   "));
        // The query is trimmed before matching.
        assert!(pose_name_matches("Wave Pose", "  wave  "));
    }

    #[test]
    fn visible_pose_entries_keep_real_indices() {
        let names = vec![
            "Alpha".to_string(),
            "Beta".to_string(),
            "alpha strike".to_string(),
        ];
        // Empty query: every entry, in order.
        assert_eq!(visible_pose_entries(&names, ""), vec![0, 1, 2]);
        assert_eq!(visible_pose_entries(&names, "   "), vec![0, 1, 2]);
        // Filtered: the surviving entries keep their
        // indices into the full list, so row keys and
        // handlers still address the same poses.
        assert_eq!(visible_pose_entries(&names, "alpha"), vec![0, 2]);
        assert_eq!(visible_pose_entries(&names, "ALPHA"), vec![0, 2]);
        assert_eq!(visible_pose_entries(&names, "Beta"), vec![1]);
        // No match: no rows.
        assert!(visible_pose_entries(&names, "qqq").is_empty());
    }

    #[test]
    fn pose_count_line_is_the_bare_total_without_a_filter() {
        // Byte-identical discipline to figures_panel: the
        // idle count is the bare number.
        assert_eq!(pose_count_line(3, 3, ""), "3");
        assert_eq!(pose_count_line(3, 3, "   "), "3");
        assert_eq!(pose_count_line(0, 0, ""), "0");
    }

    #[test]
    fn pose_count_line_shows_shown_of_total_with_a_filter() {
        assert_eq!(pose_count_line(2, 3, "alpha"), "2 of 3 poses");
        assert_eq!(pose_count_line(0, 3, "qqq"), "0 of 3 poses");
    }

    #[test]
    fn pose_filter_active_only_for_a_real_query() {
        assert!(!pose_filter_active(""));
        assert!(!pose_filter_active("   "));
        assert!(pose_filter_active("a"));
        assert!(pose_filter_active("  a  "));
    }

    #[test]
    fn apply_mirrored_glyph_is_the_flip_glyph() {
        // Phase I pin: the honest flip glyph replaced
        // Phase H's `copy` stand-in, and the name must
        // resolve in the catalogue or icon_btn would
        // silently degrade to a plain text button.
        assert_eq!(APPLY_MIRRORED_ICON, "arrow-right-arrow-left");
        assert!(crate::rune_sec::icons::has_icon(APPLY_MIRRORED_ICON));
    }

    #[test]
    fn apply_to_all_frames_writes_every_frame_as_one_undo_step() {
        let mut c = core();
        let mut clip = None;
        // Make frame 0's pose distinctive, then copy it.
        c.mirror_selected_pose();
        let mirrored = selected_pose(&c).unwrap().clone();
        copy_pose(&mut c, &mut clip);
        assert!(clip.is_some());
        // The state one undo must restore afterwards:
        // frame 0 mirrored, every other frame untouched.
        let pre_apply: Vec<Pose> = c
            .project
            .frames
            .iter()
            .map(|f| f.poses[0].clone())
            .collect();
        let current = c.current;
        let depth = c.undo_depth();
        apply_pose_to_all_frames(&mut c, &clip);
        assert_eq!(
            c.undo_depth(),
            depth + 1,
            "the whole batch must be exactly one undo step"
        );
        assert_eq!(c.current, current, "the current frame never moves");
        for f in &c.project.frames {
            assert_eq!(f.poses[0].angles, mirrored.angles);
            assert_eq!(f.poses[0].root, mirrored.root);
        }
        assert_eq!(
            c.status,
            format!(
                "Pose applied to {} frames (0 skipped).",
                c.frame_count()
            )
        );
        assert_eq!(
            crate::rune_sec::toasts::classify_status(&c.status),
            crate::rune_sec::toasts::Severity::Info,
            "a full application stays a plain confirmation"
        );
        c.undo();
        for (f, before) in c.project.frames.iter().zip(&pre_apply) {
            assert_eq!(f.poses[0].angles, before.angles);
            assert_eq!(f.poses[0].root, before.root);
        }
    }

    #[test]
    fn apply_to_all_frames_skips_missing_and_mismatched_rows() {
        let mut c = core();
        let mut clip = None;
        c.mirror_selected_pose();
        copy_pose(&mut c, &mut clip);
        let n = c.frame_count();
        assert!(n >= 4, "demo project needs at least four frames");
        // Frame 1 holds no pose row for the figure at
        // all; frame 3's pose gains an angle the copied
        // rig does not have. Both must be skipped, and
        // frame 3's row must survive byte for byte.
        c.project.frames[1].poses.clear();
        c.project.frames[3].poses[0].angles.push(0.25);
        let odd = c.project.frames[3].poses[0].clone();
        let depth = c.undo_depth();
        apply_pose_to_all_frames(&mut c, &clip);
        assert_eq!(c.undo_depth(), depth + 1);
        assert_eq!(
            c.status,
            format!(
                "Pose applied to {} frames; 2 cannot take the copied pose (no pose row or a different rig).",
                n - 2
            )
        );
        // The partial application reads as a warning,
        // not a plain confirmation (Wave 10 audit D7).
        assert_eq!(
            crate::rune_sec::toasts::classify_status(&c.status),
            crate::rune_sec::toasts::Severity::Warn,
            "a partial application classifies as Warn"
        );
        assert!(
            c.project.frames[1].poses.is_empty(),
            "a missing pose row stays missing"
        );
        assert_eq!(c.project.frames[3].poses[0].angles, odd.angles);
        assert_eq!(c.project.frames[3].poses[0].root, odd.root);
        let clip_pose = clip.as_ref().unwrap();
        for (i, f) in c.project.frames.iter().enumerate() {
            if i == 1 || i == 3 {
                continue;
            }
            assert_eq!(f.poses[0].angles, clip_pose.angles);
            assert_eq!(f.poses[0].root, clip_pose.root);
        }
    }

    #[test]
    fn apply_to_all_frames_with_no_writable_frame_reports_a_refusal() {
        // Wave 9 audit D7: with a pose copied but no
        // frame able to take it, the batch changes
        // nothing — no snapshot — and the status must
        // read as the refusal it is (the classifier's
        // "No …" warning family), not in the success
        // sentence's shape.
        let mut c = core();
        let mut clip = None;
        copy_pose(&mut c, &mut clip);
        assert!(clip.is_some());
        for f in &mut c.project.frames {
            f.poses.clear();
        }
        let depth = c.undo_depth();
        apply_pose_to_all_frames(&mut c, &clip);
        assert_eq!(c.undo_depth(), depth, "no writable frame, no snapshot");
        assert!(
            c.status.starts_with("No frame could take the copied pose"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn apply_to_all_frames_without_copy_is_a_status_only_noop() {
        let mut c = core();
        let before = selected_pose(&c).unwrap().clone();
        let depth = c.undo_depth();
        apply_pose_to_all_frames(&mut c, &None);
        assert_eq!(c.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Nothing copied"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn mirror_all_gate_is_mirror_poses_gate() {
        // Both Mirror pose and Mirror figure (all
        // frames) render on `has_selected_pose` — the
        // same expression in `pose_tools` — so the
        // all-frames button is live exactly when its
        // single-frame parent is: on the demo, and inert
        // once the current frame holds no pose for the
        // selected figure.
        let mut c = core();
        assert!(has_selected_pose(&c));
        c.project.frames[c.current].poses.clear();
        assert!(!has_selected_pose(&c));
    }
}

