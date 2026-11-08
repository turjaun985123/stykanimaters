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
//! - **Copy pose to…** (one button per other figure) →
//!   `EditorCore::copy_pose_between(selected, target)`
//!   (editor.rs:1438), reading the selection live at press
//!   time.
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
//! copied pose *and* a target pose, copy-to needs both poses,
//! bake needs at least one graph channel and one frame.
//! Library apply and mirror-apply need a pose to land on;
//! library delete/rename act on files, not on the frame, so
//! they are always live.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, text_input, El, TextEdit, Tx};
use serde::{Deserialize, Serialize};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

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

/// Snapshot once and write `pose` over the selected figure's
/// current-frame pose — the single write path Paste pose and
/// library Apply share, so both are exactly one undo step.
/// Callers guard pose existence and angle count first.
fn write_current_pose(core: &mut EditorCore, pose: Pose) {
    core.snapshot();
    let (fi, fr) = (core.selected_figure, core.current);
    core.project.frames[fr].poses[fi] = pose;
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

/// The directory the 2D pose library lives in:
/// `./library/poses`, inside the 2D figure library's own
/// directory — where the 3D mirror keeps `./library3d/poses`
/// beside its figure library.
fn pose_library_dir() -> PathBuf {
    PathBuf::from("library").join("poses")
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

/// Load the pose saved under `name` from pose library
/// directory `dir`. An error is returned when no readable,
/// well-formed entry exists for that name (missing file,
/// invalid JSON, wrong shape).
fn load_pose_library_in(dir: &Path, name: &str) -> std::io::Result<Pose> {
    let path = dir.join(pose_file_name(name));
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
            core.status = format!("Pose saved to the library as '{name}'.");
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
    let path = dir.join(pose_file_name(name));
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
    let old_path = dir.join(pose_file_name(old));
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
            // exactly that; the session is settled.
            core.status = format!(
                "Pose '{old}' renamed to '{new_name}', but the old file could not be removed ({e})."
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
            text_input(&s.pose_rename.field, type_::BODY.lh(26.0))
                .key("pose-rename")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder("New name", t.text_secondary)
                .on_edit(|s: &mut State| &mut s.pose_rename.field)
                .on_submit(|s: &mut State, _tx: &mut Tx| apply_rename(s)),
        )
        .child(pbtn(
            "pose-rename-apply".to_string(),
            "Apply",
            true,
            t,
            |s| apply_rename(s),
        ))
        .child(pbtn(
            "pose-rename-cancel".to_string(),
            "Cancel",
            true,
            t,
            |s| cancel_rename(s),
        ))
}

/// One pose-tools button, in the chrome's metrics
/// (`chrome_view::btn`): padded rounded label. When
/// `enabled` is false the button renders muted and carries
/// NO press handler — an inert element, not a control that
/// silently does nothing.
fn pbtn(
    key: String,
    label: &str,
    enabled: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let base = el().key(key).px(10.0).py(5.0).radius(6.0);
    if enabled {
        base.fill(t.card)
            .hover(t.control_track)
            .press(t.control_off)
            .on_press(f)
            .child(text(label).font(type_::LABEL).color(t.text))
    } else {
        base.fill(t.control_off)
            .child(text(label).font(type_::LABEL).color(t.text_secondary))
    }
}

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
    let mut body = col()
        .key("pose-tools")
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Pose tools")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(el().grow(1.0))
                .child(text(caption).font(type_::LABEL).color(t.text_secondary)),
        )
        .child(
            row()
                .gap(4.0)
                .child(pbtn(
                    "pose-mirror".to_string(),
                    "Mirror pose",
                    has_pose,
                    t,
                    |s| s.core.mirror_selected_pose(),
                ))
                .child(pbtn(
                    "pose-copy".to_string(),
                    "Copy pose",
                    has_pose,
                    t,
                    |s| copy_pose(&mut s.core, &mut s.pose_clipboard),
                ))
                .child(pbtn(
                    "pose-paste".to_string(),
                    "Paste pose",
                    can_paste,
                    t,
                    |s| paste_pose(&mut s.core, &s.pose_clipboard),
                )),
        )
        .child(
            row()
                .gap(4.0)
                .child(pbtn(
                    "pose-reset".to_string(),
                    "Reset pose",
                    has_pose,
                    t,
                    |s| reset_pose(&mut s.core),
                ))
                .child(pbtn(
                    "pose-bake-graph".to_string(),
                    "Bake graph",
                    can_bake(core),
                    t,
                    |s| {
                        let last = s.core.frame_count().saturating_sub(1);
                        s.core.bake_graph_channels(0, last);
                    },
                )),
        )
        .child(
            row().gap(4.0).child(pbtn(
                "pose-save-lib".to_string(),
                "Save pose",
                has_pose,
                t,
                |s| save_current_pose(&mut s.core),
            )),
        );
    let saved = saved_pose_names();
    if !saved.is_empty() {
        body = body.child(
            text("Pose library")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
        for (i, name) in saved.iter().enumerate() {
            let apply_name = name.clone();
            let mirror_name = name.clone();
            let del_name = name.clone();
            let mut entry = col()
                .key(format!("pose-lib-row-{i}"))
                .gap(4.0)
                .child(
                    row()
                        .gap(4.0)
                        .center_y()
                        .child(text(name.as_str()).font(type_::LABEL).color(t.text))
                        .child(el().grow(1.0))
                        .child(pbtn(
                            format!("pose-lib-rename-{i}"),
                            "Rename",
                            true,
                            t,
                            move |s: &mut State| start_rename(s, i),
                        ))
                        .child(pbtn(
                            format!("pose-lib-del-{i}"),
                            "Del",
                            true,
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
                        .child(pbtn(
                            format!("pose-lib-apply-{i}"),
                            "Apply",
                            has_pose,
                            t,
                            move |s: &mut State| {
                                apply_library_pose(&mut s.core, &apply_name)
                            },
                        ))
                        .child(pbtn(
                            format!("pose-lib-apply-mirror-{i}"),
                            "Apply mirrored",
                            has_pose,
                            t,
                            move |s: &mut State| {
                                apply_library_pose_mirrored(
                                    &mut s.core,
                                    &mirror_name,
                                )
                            },
                        )),
                );
            if s.pose_rename.target.is_some()
                && s.pose_rename.name.as_str() == name.as_str()
            {
                entry = entry.child(rename_editor_row(s, t));
            }
            body = body.child(entry);
        }
    }
    let targets = pose_targets(core);
    if !targets.is_empty() {
        body = body.child(
            text("Copy pose to")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
        let mut trow = row().gap(4.0);
        for j in targets {
            let enabled = has_pose && pose_exists(core, j);
            trow = trow.child(pbtn(
                format!("pose-copy-to-{}", j + 1),
                &figure_label(core, j),
                enabled,
                t,
                move |s: &mut State| {
                    s.core.copy_pose_between(s.core.selected_figure, j)
                },
            ));
        }
        body = body.child(trow.scroll_x().w_full());
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

