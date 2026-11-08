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
//!
//! Buttons whose op would no-op on the current state render
//! muted (secondary text, no press handler — inert, not fake):
//! mirror/copy/reset need a pose on this frame, paste needs a
//! copied pose *and* a target pose, copy-to needs both poses,
//! bake needs at least one graph channel and one frame.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
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

/// Load the library pose `name` and write it over the
/// selected figure's current-frame pose, through the same
/// path Paste pose uses ([`write_current_pose`]: one
/// snapshot, one write — one undo step), with Paste's
/// guards: no target pose or an angle count that does not
/// match this figure's rig is a status-only refusal.
pub fn apply_library_pose(core: &mut EditorCore, name: &str) {
    apply_library_pose_in(core, &pose_library_dir(), name)
}

fn apply_library_pose_in(core: &mut EditorCore, dir: &Path, name: &str) {
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
    let (fi, fr) = (core.selected_figure + 1, core.current + 1);
    write_current_pose(core, pose);
    core.status = format!("Pose '{name}' applied to figure {fi} on frame {fr}.");
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
        let mut lrow = row().gap(4.0);
        for (i, name) in saved.iter().enumerate() {
            let label = name.clone();
            let entry_name = name.clone();
            lrow = lrow.child(pbtn(
                format!("pose-lib-apply-{i}"),
                &label,
                has_pose,
                t,
                move |s: &mut State| apply_library_pose(&mut s.core, &entry_name),
            ));
        }
        body = body.child(lrow.scroll_x().w_full());
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
}

