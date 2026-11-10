//! MCP render/format tools: saving, loading, importing, exporting
//! and rendering, so an AI agent can finish a job end-to-end
//! through MCP without driving the GUI.
//!
//! All paths are plain filesystem paths the server process can
//! access. Parent directories are created where sensible (file
//! targets; the `jpeg`/`svg` render formats treat `path` as a
//! directory and create it). Handlers never panic: every failure
//! is a descriptive `Err(String)` the agent can act on.
//!
//! Wave 11 (A5): two parity/atomicity passes over this file.
//! Writes the tools perform themselves go through
//! [`write_atomic`] (temp sibling + rename), so a failed save
//! never truncates an existing project file. And the project
//! swaps below mirror the editor's own session bookkeeping per
//! format — `note_project_swapped` where the editor calls it,
//! a pre-swap `snapshot()` where the editor snapshots — see
//! [`replace_project`] and the per-format notes in
//! `tool_import_file`. Two editor bookkeeping steps have no
//! public API and stay integrator items: clearing the undo
//! history on a `.piv` import (editor.rs `import_piv` clears
//! both stacks; the stacks are private) and refreshing
//! `core.project_files` after a save (editor.rs
//! `list_project_files` is private).
//!
//! Wave 12 (C6) io-truth audit: no handler in this file formats
//! rendered output itself — every render/export below delegates
//! to the engine writers (`export::*` for the raster and video
//! formats, `svg::*` for SVG, `pivot::*` / `nodes*::*` for the
//! interchange formats), and save/load serialise the whole
//! [`Project`] with the same `serde_json::to_string_pretty`
//! call the editor's own save uses (projects.rs), so renderer
//! and model changes reach MCP output with no io-side pin to
//! drift. The two places this file touches rendered bytes at
//! all are encoders, not renderers: [`encode_png`] (the png
//! crate over the rasterizer's buffer) and the contact sheet's
//! extension-inferred `save`. The tool descriptions state
//! exactly what each output contains today; where the SVG
//! writer's fidelity is still behind the rasterizer's, the
//! `render` description says so rather than implying parity.
//!
//! Wave R (R13): frame-range exports take the task-named
//! `from_frame`/`to_frame` arguments (the Wave 11 `from`/`to`
//! spellings stay accepted for the same bounds), resolved by
//! [`arg_frame_range`] and validated strictly against the
//! timeline by [`check_frame_range`] before any file is
//! written — an export writes exactly the frames named, so a
//! bound past the end is refused instead of silently
//! exporting a shorter range. The engine's range exporters
//! keep their clamping contract for the export panel, whose
//! loop range is pre-clamped state rather than an argument;
//! the contact sheet keeps [`arg_range`] and its clamping
//! window for the same reason. `export_bundle` takes no
//! range at all: bundles cover the whole timeline on both
//! surfaces by deliberate design (see the handler).

use crate::editor::EditorCore;
use crate::mcp::Tool;
use crate::model::Project;
use image::{ImageBuffer, Rgb};
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use std::path::Path;

/// Palette the editor cycles through for figures added by import
/// (mirrors editor.rs `FIGURE_COLORS`).
const FIGURE_COLORS: [u32; 6] = [0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF];

// ---------------------------------------------------------------------
// Argument / filesystem helpers
// ---------------------------------------------------------------------

fn arg_path(args: &Value) -> Result<String, String> {
    args.get("path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "missing required string argument: path".to_string())
}

fn arg_frame(core: &EditorCore, args: &Value) -> Result<usize, String> {
    match args.get("frame") {
        None | Some(Value::Null) => Ok(core.current),
        Some(v) => v
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| "argument 'frame' must be a non-negative integer".to_string()),
    }
}

/// One range bound under either spelling: the task-named
/// `framed` key (`from_frame`/`to_frame`) or the `legacy`
/// key (`from`/`to`) the Wave 11 tools shipped with. Absent
/// (or null) under both spellings is `None`; a non-integer
/// under either is refused naming that key (floats and
/// negatives included — the bounds must be exact frame
/// indices); both spellings present must agree, since a
/// disagreement is a caller mistake, not a choice.
fn arg_range_bound(args: &Value, legacy: &str, framed: &str) -> Result<Option<usize>, String> {
    let mut found: Option<usize> = None;
    for key in [legacy, framed] {
        match args.get(key) {
            None | Some(Value::Null) => {}
            Some(v) => {
                let n = v.as_u64().map(|n| n as usize).ok_or_else(|| {
                    format!("argument '{key}' must be a non-negative integer")
                })?;
                match found {
                    Some(prev) if prev != n => {
                        return Err(format!(
                            "arguments '{legacy}' and '{framed}' disagree for the same range bound"
                        ));
                    }
                    _ => found = Some(n),
                }
            }
        }
    }
    Ok(found)
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create directory {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}

/// A sibling temp path for an atomic write to `path`: same
/// directory (so the rename never crosses filesystems), same
/// extension (so writers that infer format from the extension
/// behave identically), and a process-unique marker so two
/// saves in one process can never collide.
fn temp_sibling(path: &Path) -> std::path::PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    let marker = format!("{stem}.tmp-{}", std::process::id());
    match path.extension() {
        Some(ext) => path.with_file_name(format!("{marker}.{}", ext.to_string_lossy())),
        None => path.with_file_name(marker),
    }
}

/// Move a fully written temp file over its target. On Unix the
/// rename replaces atomically; the remove-then-rename fallback
/// only runs where rename refuses an existing target
/// (Windows), where a direct overwrite was never atomic anyway.
fn rename_over(tmp: &Path, path: &Path) -> Result<(), String> {
    match std::fs::rename(tmp, path) {
        Ok(()) => Ok(()),
        Err(first) => {
            if path.exists() {
                let _ = std::fs::remove_file(path);
                std::fs::rename(tmp, path).map_err(|e| {
                    let _ = std::fs::remove_file(tmp);
                    format!("could not write {}: {e}", path.display())
                })
            } else {
                let _ = std::fs::remove_file(tmp);
                Err(format!("could not write {}: {first}", path.display()))
            }
        }
    }
}

/// Write `bytes` to `path` atomically: serialise to a temp
/// sibling, flush it to disk, then rename it over the target.
/// Any failure leaves a pre-existing target byte-for-byte
/// intact and no temp file behind — a crashed or failed save
/// can never truncate the user's only copy of a project. (The
/// editor's own `save_named_project_to` writes in place, so
/// this is strictly stronger than the GUI's save; the export
/// and render writers inside export.rs keep their own write
/// paths and are out of this file's reach.)
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = temp_sibling(path);
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("could not write {}: {e}", path.display()));
    }
    rename_over(&tmp, path)
}

/// Replace the session project and reset selection/playhead the
/// same way the editor does when it opens a project file. The
/// two wrappers below add the per-path session bookkeeping the
/// editor pairs with the swap; callers pick the one matching
/// their editor analogue. (The editor also leaves `solo` reset
/// and the playhead rewound in every swap path — both are done
/// here.)
fn replace_project(core: &mut EditorCore, project: Project) {
    core.project = project;
    core.current = 0;
    core.playhead = 0.0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
    // Wave 11 (E9): count the swap and repair the clip-edit
    // session against the new project — the GUI shell's poll
    // watches the counter to drop stale shell-side selection
    // state after an MCP-driven swap.
    core.note_project_replaced();
}

/// The swap as an OPEN: mirrors `EditorCore::open_project_file` /
/// `open_via_dialog` — the dirty baseline restarts with the
/// newly opened project (`note_project_swapped`, so the session
/// no longer reads dirty for the previous project's unsaved
/// edits) and the status line names the file, which is also how
/// a live-control user sees that MCP replaced their project.
/// Like the editor's open, the undo history is left in place.
fn replace_project_open(core: &mut EditorCore, project: Project, path: &str) {
    replace_project(core, project);
    core.note_project_swapped();
    core.status = format!("Opened {path}");
}

/// The swap as a `.piv` IMPORT: mirrors `EditorCore::import_piv`
/// — same baseline restart, same status wording, and the same
/// undo-history clear (`EditorCore::clear_history`, added for
/// exactly this parity in Wave 11).
fn replace_project_import(core: &mut EditorCore, project: Project, path: &str) {
    replace_project(core, project);
    core.clear_history();
    core.note_project_swapped();
    core.status = format!("Imported {path}");
}

fn summary(core: &EditorCore) -> Value {
    let p = &core.project;
    json!({
        "name": p.name,
        "figures": p.figures.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
        "figure_count": p.figures.len(),
        "frames": p.frames.len(),
        "fps": p.fps,
        "duration_seconds": p.total_seconds(),
        "stage": p.stage,
        "has_audio": !p.audio.is_empty(),
        "audio_clips": p.audio.len(),
        "weather": serde_json::to_value(&p.weather).unwrap_or(Value::Null),
        "camera_keys": p.camera.len(),
    })
}

// ---------------------------------------------------------------------
// save / load
// ---------------------------------------------------------------------

fn tool_save_project(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let path = Path::new(&path);
    ensure_parent(path)?;
    let text = serde_json::to_string_pretty(&core.project)
        .map_err(|e| format!("could not serialise project: {e}"))?;
    // Atomic: a failed save never truncates an existing file.
    write_atomic(path, text.as_bytes())?;
    // Save bookkeeping, mirroring `save_named_project_to`'s
    // success arm: the saved state IS the dirty baseline, so
    // the session stops reading dirty for work that is on
    // disk, and the status line names the file (how a
    // live-control user sees the save). The core exposes no
    // save-baseline setter — `note_project_swapped` zeroes
    // both edit counters, which is observably identical here
    // because every reader of the counters
    // (`edits_since_autosave`, the autosave threshold) is
    // difference-based. The `project_files` refresh — the
    // step Wave 11 could not mirror (`list_project_files`
    // was private then) — is mirrored as of Wave 12 through
    // the `projects::refresh_project_files` seam (B3).
    core.note_project_swapped();
    crate::projects::refresh_project_files(core);
    core.status = format!("Project saved to {}", path.display());
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": text.len(),
        "name": core.project.name,
    }))
}

fn tool_load_project(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))?;
    let project: Project = serde_json::from_str(&text)
        .map_err(|e| format!("{path} is not a valid project JSON: {e}"))?;
    replace_project_open(core, project, &path);
    // Wave 11 (E9): point the file selector at the opened file
    // and record the open in the recents list, as a GUI open
    // does — without these, Home's current-marking and Reopen
    // suppression lied after every MCP load.
    core.note_project_loaded(Path::new(&path));
    crate::shell::touch_recent(&core.project.name);
    let mut out = summary(core);
    out["path"] = json!(path);
    Ok(out)
}

// ---------------------------------------------------------------------
// import / export
// ---------------------------------------------------------------------

fn tool_import_file(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read {path}: {e}"))?;
    // Per-format session bookkeeping mirrors the editor's own
    // import op for that format (Wave 11 A5 parity pass):
    // `import_piv` restarts the dirty baseline, the Stick
    // Nodes imports snapshot first so the import is a single
    // undoable edit, and `.json` is an open, not an import.
    match ext.as_str() {
        "piv" => {
            let project = crate::pivot::import_piv(&bytes)?;
            replace_project_import(core, project, &path);
            core.note_project_loaded(Path::new(&path));
            crate::shell::touch_recent(&core.project.name);
        }
        "nodemc" | "stknds" => {
            let project = crate::nodes_anim::import_nodes_animation(&bytes)?;
            // editor.rs `import_nodes_anim_file`: snapshot
            // first (the swap is one undo step), no baseline
            // restart — the snapshot counts as an edit.
            core.snapshot();
            replace_project(core, project);
            core.status = format!("Imported {path}.");
        }
        "nodes" => {
            // nodes::import_nodes wraps import_nodes_figure into a
            // one-figure project, exactly as the editor imports it
            // (`import_nodes_file`: snapshot first, same rule as
            // the animation import above).
            let project = crate::nodes::import_nodes(&bytes)?;
            core.snapshot();
            replace_project(core, project);
            core.status = format!("Imported {path} as a figure.");
        }
        "json" => {
            let text =
                String::from_utf8(bytes).map_err(|_| format!("{path} is not valid UTF-8 JSON"))?;
            let project: Project = serde_json::from_str(&text)
                .map_err(|e| format!("{path} is not a valid project JSON: {e}"))?;
            replace_project_open(core, project, &path);
            core.note_project_loaded(Path::new(&path));
            crate::shell::touch_recent(&core.project.name);
        }
        "stk" => {
            // A .stk is a single figure: like the editor, it is
            // added to the current project rather than replacing it.
            let (def, name) = crate::pivot::import_stk(&bytes)?;
            // editor.rs `import_stk`: snapshot first (adding
            // the figure is undoable), and the figure lands at
            // the editor's fixed import root [320, 450] — NOT
            // the stage centre this tool used before, which put
            // MCP-imported figures in a different place than
            // GUI-imported ones.
            core.snapshot();
            let idx = core.project.figures.len();
            let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
            let label = if name.is_empty() { "Imported figure".to_string() } else { name };
            core.project.add_figure_with(def, &label, color, [320.0, 450.0]);
            core.selected_figure = idx;
            core.status = format!("Imported figure from {path}");
            let mut out = summary(core);
            out["imported_format"] = json!("stk");
            out["added_figure"] = json!(label);
            out["path"] = json!(path);
            return Ok(out);
        }
        _ => {
            return Err(format!(
                "unsupported import extension '.{ext}' for {path} \
                 (supported: piv, stk, nodes, nodemc, stknds, json)"
            ));
        }
    }
    let mut out = summary(core);
    out["imported_format"] = json!(ext);
    out["path"] = json!(path);
    Ok(out)
}

/// The figure index for single-figure exports: the `figure`
/// argument, defaulting to the editor's selected figure.
fn arg_figure(core: &EditorCore, args: &Value) -> Result<usize, String> {
    let idx = match args.get("figure") {
        None | Some(Value::Null) => core.selected_figure,
        Some(v) => v.as_u64().map(|n| n as usize).ok_or_else(|| {
            "argument 'figure' must be a non-negative integer".to_string()
        })?,
    };
    if idx < core.project.figures.len() {
        Ok(idx)
    } else {
        Err(format!(
            "figure index {idx} out of range (project has {} figures)",
            core.project.figures.len()
        ))
    }
}

fn tool_export_file(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument: format".to_string())?;
    if format == "mp4_range" || format == "webm_range" {
        // Frame-range video exports write through ffmpeg like
        // export_mp4/export_webm, so they do not fit the byte
        // pipeline below: they render straight to `path` and
        // return the number of frames written.
        // Bounds are required for the range formats: the
        // task-named 'from_frame'/'to_frame' or the Wave 11
        // 'from'/'to' spellings (Wave R, R13), validated
        // against the timeline before any file is written.
        let (from, to) = match arg_frame_range(args)? {
            Some((f, t, framed)) => {
                check_frame_range((f, t), core.project.frames.len(), framed)?;
                (f, t)
            }
            None => {
                return Err("missing required integer argument: from".to_string());
            }
        };
        let path_ref = Path::new(&path);
        ensure_parent(path_ref)?;
        let mut missing_audio: Vec<String> = Vec::new();
        if format == "mp4_range" {
            let outcome = crate::export::export_mp4_range(&core.project, from, to, path_ref)
                .map_err(|e| format!("render to {path} failed: {e}"))?;
            missing_audio = outcome.missing_audio;
        } else {
            crate::export::export_webm_range(&core.project, from, to, path_ref)
                .map_err(|e| format!("render to {path} failed: {e}"))?;
        }
        let mut out = json!({
            "path": path,
            "format": format,
            "from": from,
            "to": to,
        });
        // Wave P (P15): an MP4 range export that had to
        // leave out clips whose files are missing names
        // them here; with every clip present (and for
        // WebM, which carries no audio) the reply keeps
        // the shape it always had.
        if !missing_audio.is_empty() {
            out["missing_audio"] = json!(missing_audio);
        }
        return Ok(out);
    }
    let bytes: Vec<u8> = match format {
        "piv2" => crate::pivot::export_piv_v2(&core.project),
        "piv3" => crate::pivot::export_piv_v3(&core.project),
        "piv4" => crate::pivot::export_piv_v4(&core.project),
        "piv5" => crate::pivot::export_piv_v5(&core.project),
        "stk" => {
            let fig = &core.project.figures[arg_figure(core, args)?];
            crate::pivot::export_stk(&fig.def, &fig.name)
        }
        "nodes" => {
            let fig = &core.project.figures[arg_figure(core, args)?];
            crate::nodes::export_nodes_figure(&fig.def, &fig.name)
        }
        "stknds" => crate::nodes_anim::export_nodes_animation(&core.project),
        _ => {
            return Err(format!(
                "unknown export format '{format}' (supported: piv2, piv3, piv4, piv5, stk, nodes, stknds)"
            ));
        }
    };
    let path_ref = Path::new(&path);
    ensure_parent(path_ref)?;
    write_atomic(path_ref, &bytes)?;
    Ok(json!({
        "path": path,
        "format": format,
        "bytes": bytes.len(),
    }))
}

// ---------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------

/// Encode an RGB buffer as PNG bytes in memory (png crate).
fn encode_png(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), img.width(), img.height());
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG encode failed: {e}"))?;
        writer
            .write_image_data(img.as_raw())
            .map_err(|e| format!("PNG encode failed: {e}"))?;
    }
    Ok(out)
}

/// Render one authored frame at the requested output scale
/// (1.0 = exact stage size): a true re-render at that scale
/// through the export funnel (Wave R, R10 via IA-R10-1),
/// never a resample of the 1.0 raster.
fn render_png_bytes(
    core: &EditorCore,
    frame: usize,
    scale: f64,
) -> Result<(Vec<u8>, u32, u32), String> {
    let p = &core.project;
    if p.frames.is_empty() {
        return Err("project has no frames to render".to_string());
    }
    if frame >= p.frames.len() {
        return Err(format!(
            "frame {frame} out of range (project has {} frames)",
            p.frames.len()
        ));
    }
    if !(scale > 0.0) || !scale.is_finite() {
        return Err("argument 'scale' must be a positive number".to_string());
    }
    let img = crate::export::render_frame_at_scaled(p, frame as f32, scale as f32)
        .map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    Ok((encode_png(&img)?, w, h))
}

/// An optional frame range from 'from'/'to' arguments: both
/// or neither (Wave 11, C9 G1) — a lone bound is almost
/// certainly a caller mistake, so it is an error rather than
/// a silent half-range. Indices are 0-based and inclusive;
/// the engine's range exporters clamp them to the timeline.
fn arg_range(args: &Value) -> Result<Option<(usize, usize)>, String> {
    let from = match args.get("from") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| "argument 'from' must be a non-negative integer".to_string())?,
        ),
    };
    let to = match args.get("to") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| "argument 'to' must be a non-negative integer".to_string())?,
        ),
    };
    match (from, to) {
        (Some(f), Some(t)) => {
            if t < f {
                Err("argument 'to' must not be less than 'from'".to_string())
            } else {
                Ok(Some((f, t)))
            }
        }
        (None, None) => Ok(None),
        _ => Err("arguments 'from' and 'to' must be given together".to_string()),
    }
}

/// A frame range for the EXPORT tools (Wave R, R13): the
/// task-named 'from_frame'/'to_frame' arguments, with the
/// Wave 11 'from'/'to' spellings still accepted for the same
/// bounds via [`arg_range_bound`]. Both bounds or neither —
/// a lone bound is refused, as in [`arg_range`]. The third
/// element records whether a 'from_frame'/'to_frame' spelling
/// was used, so error messages and reply echoes speak the
/// caller's vocabulary. Timeline validation is the caller's
/// next step ([`check_frame_range`]): this resolver does not
/// know the frame count.
fn arg_frame_range(args: &Value) -> Result<Option<(usize, usize, bool)>, String> {
    let from = arg_range_bound(args, "from", "from_frame")?;
    let to = arg_range_bound(args, "to", "to_frame")?;
    let framed = matches!(args.get("from_frame"), Some(v) if !v.is_null())
        || matches!(args.get("to_frame"), Some(v) if !v.is_null());
    let (from_name, to_name) = if framed {
        ("from_frame", "to_frame")
    } else {
        ("from", "to")
    };
    match (from, to) {
        (Some(f), Some(t)) => {
            if t < f {
                Err(format!(
                    "argument '{to_name}' must not be less than '{from_name}'"
                ))
            } else {
                Ok(Some((f, t, framed)))
            }
        }
        (None, None) => Ok(None),
        _ => Err(format!(
            "arguments '{from_name}' and '{to_name}' must be given together"
        )),
    }
}

/// The export tools' strict timeline check (Wave R, R13),
/// run before any file is written: the last frame of the
/// range must exist. The engine's range exporters clamp
/// out-of-range bounds (their contract with the panel, whose
/// loop range is pre-clamped state), but an MCP caller who
/// names frame 40 of a 24-frame project asked for frames
/// that do not exist — silently writing a shorter export
/// would misreport what was produced, so the range is
/// refused in the same voice as `render_png_bytes`' frame
/// check. `framed` selects the argument vocabulary the
/// error names.
fn check_frame_range(range: (usize, usize), frames: usize, framed: bool) -> Result<(), String> {
    let to = range.1;
    if to >= frames {
        let to_name = if framed { "to_frame" } else { "to" };
        return Err(format!(
            "argument '{to_name}' {to} is out of range (project has {frames} frames)"
        ));
    }
    Ok(())
}

fn tool_render(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument: format".to_string())?;
    let project = &core.project;
    if project.frames.is_empty() {
        return Err("project has no frames to render".to_string());
    }
    // Wave R (R13): ranges arrive as 'from_frame'/'to_frame'
    // (or the Wave 11 'from'/'to' spellings) and are validated
    // against the timeline before anything is written.
    let (range, range_framed) = match arg_frame_range(args)? {
        Some((f, t, framed)) => {
            check_frame_range((f, t), project.frames.len(), framed)?;
            (Some((f, t)), framed)
        }
        None => (None, false),
    };
    if range.is_some() && matches!(format, "png" | "mp4" | "webm") {
        // 'png' is a single frame by definition; video ranges
        // already exist as export_file's mp4_range/webm_range.
        return Err(format!(
            "arguments 'from_frame'/'to_frame' are not supported for format '{format}' \
             (ranges: gif, apng, spritesheet, svg, jpeg, png_sequence; \
             for video ranges use export_file with mp4_range/webm_range)"
        ));
    }
    let path_ref = Path::new(&path);
    // Wave P (P15): the MP4 arm fills this with the clips
    // the export had to leave out (missing files); it
    // surfaces as a reply field below, only when non-empty.
    let mut mp4_missing_audio: Vec<String> = Vec::new();
    let frames = match format {
        "png" => {
            let frame = arg_frame(core, args)?;
            let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
            let (bytes, w, h) = render_png_bytes(core, frame, scale)?;
            ensure_parent(path_ref)?;
            write_atomic(path_ref, &bytes)?;
            return Ok(json!({
                "path": path,
                "format": "png",
                "frame": frame,
                "width": w,
                "height": h,
                "bytes": bytes.len(),
            }));
        }
        "gif" => {
            ensure_parent(path_ref)?;
            match range {
                Some((f, t)) => crate::export::export_gif_range(project, f, t, path_ref),
                None => crate::export::export_gif(project, path_ref),
            }
        }
        "mp4" => {
            ensure_parent(path_ref)?;
            crate::export::export_mp4(project, path_ref).map(|outcome| {
                mp4_missing_audio = outcome.missing_audio;
                outcome.frames
            })
        }
        "webm" => {
            ensure_parent(path_ref)?;
            crate::export::export_webm(project, path_ref)
        }
        "apng" => {
            ensure_parent(path_ref)?;
            match range {
                Some((f, t)) => crate::export::export_apng_range(project, f, t, path_ref),
                None => crate::export::export_apng(project, path_ref),
            }
        }
        "spritesheet" => {
            ensure_parent(path_ref)?;
            match range {
                Some((f, t)) => crate::export::export_sheet_range(project, f, t, path_ref),
                None => crate::export::export_spritesheet(project, path_ref),
            }
        }
        "svg" => {
            // svg::export_svg writes one .svg per authored frame
            // into a directory; `path` names that directory.
            std::fs::create_dir_all(path_ref)
                .map_err(|e| format!("could not create directory {path}: {e}"))?;
            match range {
                Some((f, t)) => crate::svg::export_svg_range(project, f, t, path_ref),
                None => crate::svg::export_svg(project, path_ref),
            }
        }
        "jpeg" => {
            // export_jpeg_sequence writes frame-NNNN.jpg files into
            // a directory (creating it); `path` names that directory.
            match range {
                Some((f, t)) => {
                    crate::export::export_jpeg_sequence_range(project, f, t, path_ref)
                }
                None => crate::export::export_jpeg_sequence(project, path_ref),
            }
        }
        "png_sequence" => {
            // export_png_sequence writes frame-NNNN.png files into
            // a directory (creating it); `path` names that directory.
            // Wave R (R10 via IA-R10-1): an optional 'scale'
            // argument routes to the true-scaled sequence
            // entries; absent, the unscaled calls run unchanged.
            let scale = args.get("scale").and_then(Value::as_f64);
            match (range, scale) {
                (Some((f, t)), Some(s)) => crate::export::export_png_sequence_range_scaled(
                    project,
                    f,
                    t,
                    path_ref,
                    s as f32,
                ),
                (Some((f, t)), None) => {
                    crate::export::export_png_sequence_range(project, f, t, path_ref)
                }
                (None, Some(s)) => {
                    crate::export::export_png_sequence_scaled(project, path_ref, s as f32)
                }
                (None, None) => crate::export::export_png_sequence(project, path_ref),
            }
        }
        _ => {
            return Err(format!(
                "unknown render format '{format}' \
                 (supported: png, png_sequence, gif, mp4, webm, apng, svg, spritesheet, jpeg)"
            ));
        }
    }
    .map_err(|e| format!("render to {path} failed: {e}"))?;
    let mut out = json!({
        "path": path,
        "format": format,
        "frames": frames,
    });
    if matches!(format, "svg" | "jpeg" | "png_sequence") {
        out["directory"] = json!(path);
    }
    // The range is echoed only when one was passed, so the
    // no-range reply shapes stay byte-identical (D1's pins);
    // the echo uses the spelling the caller used (Wave R,
    // R13), so a 'from'/'to' reply is unchanged from Wave 11.
    if let Some((f, t)) = range {
        if range_framed {
            out["from_frame"] = json!(f);
            out["to_frame"] = json!(t);
        } else {
            out["from"] = json!(f);
            out["to"] = json!(t);
        }
    }
    // Wave P (P15): the missing-audio list rides along on
    // the same discipline — present only when the MP4
    // export actually had to skip clips, so an export
    // with all its audio present replies exactly as
    // before.
    if !mp4_missing_audio.is_empty() {
        out["missing_audio"] = json!(mp4_missing_audio);
    }
    Ok(out)
}

/// Standard-alphabet base64 with padding (no external crate).
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn tool_render_frame_base64(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = arg_frame(core, args)?;
    // Exact stage size: the agent sees the frame at the project's
    // native resolution, one pixel per stage unit.
    let (bytes, w, h) = render_png_bytes(core, frame, 1.0)?;
    Ok(json!({
        "frame": frame,
        "width": w,
        "height": h,
        "png_base64": base64_encode(&bytes),
    }))
}

fn tool_render_contact_sheet(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let project = &core.project;
    if project.frames.is_empty() {
        return Err("project has no frames to render".to_string());
    }
    let total = project.frames.len();
    let cols = match args.get("cols") {
        None | Some(Value::Null) => 4usize,
        Some(v) => v
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| "argument 'cols' must be a positive integer".to_string())?,
    };
    if cols < 1 {
        return Err("argument 'cols' must be at least 1".to_string());
    }
    let thumb_width = match args.get("thumb_width") {
        None | Some(Value::Null) => 240u32,
        Some(v) => v
            .as_u64()
            .map(|n| n as u32)
            .ok_or_else(|| "argument 'thumb_width' must be an integer in 16..=1024".to_string())?,
    };
    if !(16..=1024).contains(&thumb_width) {
        return Err("argument 'thumb_width' must be in 16..=1024".to_string());
    }
    // Frame selection: an explicit list (out-of-range indices are
    // clamped to the last frame and reported in a note) or, by
    // default, up to 12 evenly spaced frames across the timeline
    // — or across the 'from'/'to' window when one is given
    // (Wave 11, C9 G4). 'frames' and 'from'/'to' are two ways of
    // saying the same thing, so passing both is an error.
    let range = arg_range(args)?;
    if range.is_some() && !matches!(args.get("frames"), None | Some(Value::Null)) {
        return Err("pass either 'frames' or 'from'/'to', not both".to_string());
    }
    let mut clamped = 0usize;
    let frames: Vec<usize> = match args.get("frames") {
        None | Some(Value::Null) => match range {
            Some((f, t)) => {
                let lo = f.min(total - 1);
                let hi = t.min(total - 1);
                let span = hi - lo + 1;
                let n = span.min(12);
                (0..n)
                    .map(|i| if n == 1 { lo } else { lo + i * (span - 1) / (n - 1) })
                    .collect()
            }
            None => {
                let n = total.min(12);
                (0..n)
                    .map(|i| if n == 1 { 0 } else { i * (total - 1) / (n - 1) })
                    .collect()
            }
        },
        Some(v) => {
            let arr = v
                .as_array()
                .ok_or_else(|| "argument 'frames' must be an array of frame indices".to_string())?;
            if arr.is_empty() {
                return Err("argument 'frames' must not be empty".to_string());
            }
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                let idx = item.as_u64().ok_or_else(|| {
                    "argument 'frames' entries must be non-negative integers".to_string()
                })? as usize;
                if idx >= total {
                    clamped += 1;
                }
                out.push(idx.min(total - 1));
            }
            out
        }
    };
    let sheet = crate::export::render_contact_sheet(project, &frames, cols, thumb_width);
    let (width, height) = (sheet.width(), sheet.height());
    let path_ref = Path::new(&path);
    ensure_parent(path_ref)?;
    // Same atomicity as the byte writers: encode to the temp
    // sibling (same extension, so the image crate infers the
    // same format), then rename over the target.
    let tmp = temp_sibling(path_ref);
    if let Err(e) = sheet.save(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("could not write {path}: {e}"));
    }
    rename_over(&tmp, path_ref)?;
    let mut out = json!({
        "path": path,
        "width": width,
        "height": height,
        "frames": frames,
        "cols": cols,
        "thumb_width": thumb_width,
    });
    if clamped > 0 {
        out["note"] = json!(format!(
            "{clamped} frame index(es) out of range, clamped to the last frame"
        ));
    }
    Ok(out)
}

/// One member of an export bundle: a display name, the file
/// or directory written, and the engine result (frame/file
/// count or the failure's reason).
struct BundleMember {
    name: &'static str,
    target: String,
    result: Result<usize, String>,
    /// Clips the member's export had to leave out because
    /// their files are missing (Wave P, P15). Only the MP4
    /// member can fill this — it is the bundle's only
    /// audio-carrying export — and it surfaces in the
    /// member's reply object only when non-empty.
    missing_audio: Vec<String>,
}

fn tool_export_bundle(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Wave 11 (C9 G3): the export panel's two bundles as one
    // call — kind 'images' runs PNG sequence + JPEG sequence +
    // SVG + spritesheet, kind 'video' runs MP4 + WebM, always
    // over the WHOLE timeline (the panel's bundles deliberately
    // ignore ranges, and so does this). Every member is
    // attempted; a member's failure is reported WITH its
    // reason in the reply and never fails the call itself —
    // only an unknown kind or an empty project does.
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument: kind".to_string())?;
    if !matches!(kind, "images" | "video") {
        return Err(format!(
            "unknown bundle kind '{kind}' (supported: images, video)"
        ));
    }
    let path = arg_path(args)?;
    let project = &core.project;
    if project.frames.is_empty() {
        return Err("project has no frames to render".to_string());
    }
    let dir = Path::new(&path);
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("could not create directory {path}: {e}"))?;
    // Video members are named after the project (slugified the
    // file-name way); sequence members get their own
    // subdirectories so the four image formats never collide.
    let stem = crate::projects::file_stem_for(&project.name)
        .unwrap_or_else(|| "project".to_string());
    let engine = |r: std::io::Result<usize>| r.map_err(|e| e.to_string());
    // Wave R (R10 via IA-R10-1): an optional 'scale' argument
    // scales the bundle's PNG sequence member (a true
    // re-render at that scale); absent, it renders at 1.0.
    let bundle_scale = args.get("scale").and_then(Value::as_f64);
    let members: Vec<BundleMember> = if kind == "images" {
        let png_dir = dir.join("png");
        let jpeg_dir = dir.join("jpeg");
        let svg_dir = dir.join("svg");
        let sheet = dir.join("spritesheet.png");
        vec![
            BundleMember {
                name: "png_sequence",
                target: png_dir.display().to_string(),
                result: engine(match bundle_scale {
                    Some(s) => {
                        crate::export::export_png_sequence_scaled(project, &png_dir, s as f32)
                    }
                    None => crate::export::export_png_sequence(project, &png_dir),
                }),
                missing_audio: Vec::new(),
            },
            BundleMember {
                name: "jpeg_sequence",
                target: jpeg_dir.display().to_string(),
                result: engine(crate::export::export_jpeg_sequence(project, &jpeg_dir)),
                missing_audio: Vec::new(),
            },
            BundleMember {
                name: "svg",
                target: svg_dir.display().to_string(),
                result: engine(crate::svg::export_svg(project, &svg_dir)),
                missing_audio: Vec::new(),
            },
            BundleMember {
                name: "spritesheet",
                target: sheet.display().to_string(),
                result: engine(crate::export::export_spritesheet(project, &sheet)),
                missing_audio: Vec::new(),
            },
        ]
    } else {
        let mp4 = dir.join(format!("{stem}.mp4"));
        let webm = dir.join(format!("{stem}.webm"));
        // The MP4 export's outcome is inspected before it
        // is flattened to a frame count, so the clips it
        // had to skip (missing files) ride on the member
        // and into its reply object (Wave P, P15).
        let mp4_outcome = crate::export::export_mp4(project, &mp4);
        let mp4_missing_audio = match &mp4_outcome {
            Ok(outcome) => outcome.missing_audio.clone(),
            Err(_) => Vec::new(),
        };
        vec![
            BundleMember {
                name: "mp4",
                target: mp4.display().to_string(),
                result: engine(mp4_outcome.map(|outcome| outcome.frames)),
                missing_audio: mp4_missing_audio,
            },
            BundleMember {
                name: "webm",
                target: webm.display().to_string(),
                result: engine(crate::export::export_webm(project, &webm)),
                missing_audio: Vec::new(),
            },
        ]
    };
    let member_json: Vec<Value> = members
        .iter()
        .map(|m| match &m.result {
            Ok(frames) => {
                let mut member = json!({
                    "format": m.name,
                    "path": m.target,
                    "ok": true,
                    "frames": frames,
                });
                // Wave P (P15): only when the member
                // actually skipped clips — every other
                // member object keeps its old shape.
                if !m.missing_audio.is_empty() {
                    member["missing_audio"] = json!(m.missing_audio);
                }
                member
            }
            Err(reason) => json!({
                "format": m.name,
                "path": m.target,
                "ok": false,
                "error": reason,
            }),
        })
        .collect();
    let succeeded = members.iter().filter(|m| m.result.is_ok()).count();
    Ok(json!({
        "ok": true,
        "kind": kind,
        "directory": path,
        "members": member_json,
        "succeeded": succeeded,
        "failed": members.len() - succeeded,
    }))
}

fn tool_project_summary(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    Ok(summary(core))
}

// ---------------------------------------------------------------------
// registration
// ---------------------------------------------------------------------

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "save_project",
            description: "Save the current project as StykAnimate project JSON (.styk.json format) at the given path. The COMPLETE project is serialised with the same serialiser the editor's own save uses — including the asset table (each image's PNG bytes embedded), sprite and text instances, audio clips and FX — so a save/load round-trip preserves assets byte-for-byte (pinned by a test in this module). The write is atomic (temp file + rename), so a failed save never truncates an existing file; like the editor's save, it restarts the session's dirty baseline. Parent directories are created.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination file path, e.g. /tmp/work/scene.styk.json" }
                },
                "required": ["path"]
            }),
            handler: tool_save_project,
        },
        Tool {
            name: "load_project",
            description: "Load a StykAnimate project JSON file, replacing the current session project, with the editor's open bookkeeping (selection and playhead reset to the start, dirty baseline restarted). The whole file is restored, including its asset table (embedded image bytes), sprite and text instances. Returns the project summary.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project JSON file to load" }
                },
                "required": ["path"]
            }),
            handler: tool_load_project,
        },
        Tool {
            name: "import_file",
            description: "Import a file by extension: .piv (Pivot animation), .nodemc/.stknds (Stick Nodes animation) and .nodes (Stick Nodes figure) replace the project; .stk (Pivot figure) adds the figure to the current project; .json loads a complete project JSON (asset table, sprites and texts included, exactly as load_project). Session bookkeeping matches the editor's own import for each format (the Stick Nodes and .stk imports are single undoable edits; .piv and .json restart the dirty baseline). Returns the project summary.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to import (.piv, .stk, .nodes, .nodemc, .stknds, .json)" }
                },
                "required": ["path"]
            }),
            handler: tool_import_file,
        },
        Tool {
            name: "export_file",
            description: "Export the project: 'piv2', 'piv3', 'piv4' or 'piv5' write the whole animation as .piv bytes; 'stk' writes a single figure as Pivot .stk bytes and 'nodes' writes it as a Stick Nodes .nodes figure (both take the 'figure' index, defaulting to the selected figure); 'stknds' writes the whole animation as a Stick Nodes project; 'mp4_range' and 'webm_range' render only frames 'from_frame'..='to_frame' (both required, integer frame indices; the Wave 11 spellings 'from'/'to' are still accepted for the same bounds; a bound past the end of the timeline is refused) to a video file (needs ffmpeg) with the same frame content as the render tool's raster output — composited sprite/text instances and segment media included. 'mp4_range' also muxes the range's share of the project's audio (and fails rather than writing a silent file when audio is present but unusable); 'webm_range' never carries audio — the WebM path has no audio support at any range. The .piv/.stk/.nodes/.stknds formats are data interchange: they carry only what those formats hold, not the asset table, sprites, texts or audio. Parent directories are created.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination file path" },
                    "format": { "type": "string", "enum": ["piv2", "piv3", "piv4", "piv5", "stk", "nodes", "stknds", "mp4_range", "webm_range"] },
                    "figure": { "type": "integer", "minimum": 0, "description": "Figure index for formats 'stk' and 'nodes' (default: selected figure)" },
                    "from": { "type": "integer", "minimum": 0, "description": "First frame for formats 'mp4_range' and 'webm_range' (required there)" },
                    "to": { "type": "integer", "minimum": 0, "description": "Last frame for formats 'mp4_range' and 'webm_range' (required there)" },
                    "from_frame": { "type": "integer", "minimum": 0, "description": "First frame for formats 'mp4_range' and 'webm_range' (required there; alternative spelling of 'from')" },
                    "to_frame": { "type": "integer", "minimum": 0, "description": "Last frame for formats 'mp4_range' and 'webm_range' (required there; alternative spelling of 'to')" }
                },
                "required": ["path", "format"]
            }),
            handler: tool_export_file,
        },
        Tool {
            name: "render",
            description: "Render the animation. Formats: 'png' renders ONE frame (arg 'frame', default current; arg 'scale', default 1.0 = stage size, a true re-render at that scale) to a PNG file; 'gif', 'mp4' (needs ffmpeg), 'webm', 'apng' and 'spritesheet' render the whole timeline to the file at 'path'; 'svg', 'jpeg' and 'png_sequence' write one file per frame into the DIRECTORY at 'path' (created if needed). The sequence/animation formats 'gif', 'apng', 'spritesheet', 'svg', 'jpeg' and 'png_sequence' accept an optional frame range: args 'from_frame' and 'to_frame' (0-based, inclusive, given together; the Wave 11 spellings 'from'/'to' are still accepted for the same bounds; a bound past the end of the timeline is refused rather than clamped; echoed in the reply under the spelling used). Video ranges are export_file's mp4_range/webm_range. Parent directories are created for file targets. CONTENT — the raster formats (png, png_sequence, gif, mp4, webm, apng, spritesheet, jpeg) draw the full frame: camera (pan/zoom/rotation), figures with segment styling (cap, taper, colour gradient, segment opacity AND figure opacity) and bent segments, drawing strokes, FX/particles and weather, with sprite instances, text instances and segment media (images/text attached to segments) composited in. 'mp4' also muxes the project's audio when an unmuted clip's file exists (and fails if that mux fails); 'webm' never carries audio — the WebM path has no audio support. 'svg' is the vector writer: it draws figures (segment taper, gradients, segment opacity AND figure opacity, bent segments arced through the bend midpoint), strokes (a pressured stroke as per-segment lines at its baked widths), FX/particles and weather through the same camera, and it emits sprite instances, text instances and segment media (images embedded once per asset as PNG data with a placement matrix; an image item's figure×style opacity rides as an opacity attribute). One divergence from the raster remains: SVG text is a monospace font run where the raster stamps 3×5 bitmap glyphs (the content and centre agree; the advance width does not).",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Output file path (a directory path for formats svg, jpeg and png_sequence)" },
                    "format": { "type": "string", "enum": ["png", "png_sequence", "gif", "mp4", "webm", "apng", "svg", "spritesheet", "jpeg"] },
                    "frame": { "type": "integer", "minimum": 0, "description": "Frame index for format 'png' (default: current frame)" },
                    "from": { "type": "integer", "minimum": 0, "description": "First frame of an inclusive range (with 'to'); sequence/animation formats only" },
                    "to": { "type": "integer", "minimum": 0, "description": "Last frame of an inclusive range (with 'from'); sequence/animation formats only" },
                    "from_frame": { "type": "integer", "minimum": 0, "description": "First frame of an inclusive range (with 'to_frame'); sequence/animation formats only" },
                    "to_frame": { "type": "integer", "minimum": 0, "description": "Last frame of an inclusive range (with 'from_frame'); sequence/animation formats only" },
                    "scale": { "type": "number", "exclusiveMinimum": 0, "description": "Output scale relative to the stage size (default 1.0): format 'png' is a true re-render at that scale, and format 'png_sequence' renders its sequence at that scale" }
                },
                "required": ["path", "format"]
            }),
            handler: tool_render,
        },
        Tool {
            name: "export_bundle",
            description: "Export a whole bundle of formats into the DIRECTORY at 'path' in one call (the export panel's two bundle buttons). kind 'images': a PNG sequence (png/ subdir), a JPEG sequence (jpeg/), one SVG per frame (svg/) and a spritesheet (spritesheet.png). kind 'video': '<project>.mp4' and '<project>.webm'. Member content matches the render tool per format: the image members are full raster frames (composited media included) and the SVG member matches the render tool's SVG (media included; text as font glyphs is its standing divergence); the mp4 member muxes usable project audio, the webm member is silent. Bundles always cover the WHOLE timeline — they deliberately ignore frame ranges, matching the panel. An optional 'scale' (default 1.0) scales the PNG sequence member (a true re-render at that scale). Every member is attempted: the reply lists each member with ok + frame count, or ok:false + the failure's reason; member failures never fail the call itself.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination directory (created if needed)" },
                    "kind": { "type": "string", "enum": ["images", "video"], "description": "Which bundle to export" },
                    "scale": { "type": "number", "exclusiveMinimum": 0, "description": "Output scale for the PNG sequence member relative to the stage size (default 1.0)" }
                },
                "required": ["path", "kind"]
            }),
            handler: tool_export_bundle,
        },
        Tool {
            name: "render_frame_base64",
            description: "Render one frame (default: current) as a PNG at exact stage size and return it base64-encoded, so you can SEE the frame. The PNG is the full raster frame — the same content as render format 'png', with sprite/text instances and segment media composited in. Returns {frame, width, height, png_base64}.",
            schema: json!({
                "type": "object",
                "properties": {
                    "frame": { "type": "integer", "minimum": 0, "description": "Frame index (default: current frame)" }
                }
            }),
            handler: tool_render_frame_base64,
        },
        Tool {
            name: "render_contact_sheet",
            description: "Render a contact sheet (grid of frame thumbnails) to the file at 'path' — each thumbnail is a full raster render of its frame (composited media included), and the written file's format follows the path's extension (use a .png path for a PNG; the image writer infers the format from it). Frame selection: 'frames' is an optional array of frame indices (out-of-range indices are clamped and reported in a note); or pass 'from'/'to' (0-based, inclusive, together) to draw the picks from that window; with neither, the default is up to 12 evenly spaced frames across the timeline — NOT one tile per frame (the export panel's sheet differs: it tiles every frame). 'frames' and 'from'/'to' together are an error. 'cols' is the grid column count (default 4, at least 1); 'thumb_width' is each thumbnail's width in pixels (default 240, 16..=1024). Returns {path, width, height, frames, cols, thumb_width}. Parent directories are created.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination PNG file path" },
                    "frames": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "Frame indices to include (default: up to 12 evenly spaced)" },
                    "from": { "type": "integer", "minimum": 0, "description": "Window start for the evenly spaced picks (with 'to'; not with 'frames')" },
                    "to": { "type": "integer", "minimum": 0, "description": "Window end for the evenly spaced picks (with 'from'; not with 'frames')" },
                    "cols": { "type": "integer", "minimum": 1, "description": "Grid columns (default 4)" },
                    "thumb_width": { "type": "integer", "minimum": 16, "maximum": 1024, "description": "Thumbnail width in pixels (default 240)" }
                },
                "required": ["path"]
            }),
            handler: tool_render_contact_sheet,
        },
        Tool {
            name: "project_summary",
            description: "Summarise the current project: name, figure names, frame count, fps, duration in seconds, stage size, whether it has audio, its weather setting, and the number of camera keys.",
            schema: json!({
                "type": "object",
                "properties": {}
            }),
            handler: tool_project_summary,
        },
    ]
}

// ---------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("styk-mcp-io-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tool_named(name: &str) -> Tool {
        tools()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("tool {name} not registered"))
    }

    fn call(name: &str, core: &mut EditorCore, args: Value) -> Result<Value, String> {
        (tool_named(name).handler)(core, &args)
    }

    fn base64_decode(s: &str) -> Vec<u8> {
        fn val(c: u8) -> u32 {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a') as u32 + 26,
                b'0'..=b'9' => (c - b'0') as u32 + 52,
                b'+' => 62,
                b'/' => 63,
                _ => 0,
            }
        }
        let bytes: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
        let mut out = Vec::new();
        for chunk in bytes.chunks(4) {
            let mut n: u32 = 0;
            for (i, c) in chunk.iter().enumerate() {
                n |= val(*c) << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if chunk.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }

    #[test]
    fn save_load_roundtrip_preserves_name_and_frames() {
        let dir = temp_dir("roundtrip");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        core.project.name = "MCP Roundtrip".to_string();
        let frames = core.project.frames.len();
        // Nested path: save_project must create parent directories.
        let path = dir.join("nested/deep/scene.styk.json");
        let out = call("save_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("save failed");
        assert_eq!(out["name"], "MCP Roundtrip");
        assert!(path.exists(), "saved file missing");

        core.project.name = "Changed".to_string();
        core.project.frames.clear();
        let out = call("load_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("load failed");
        assert_eq!(core.project.name, "MCP Roundtrip");
        assert_eq!(core.project.frames.len(), frames);
        assert_eq!(out["frames"], frames);
        assert_eq!(core.current, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_png_writes_real_png() {
        let dir = temp_dir("png");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("frame0.png");
        let out = call(
            "render",
            &mut core,
            json!({ "path": path.to_string_lossy(), "format": "png", "frame": 0 }),
        )
        .expect("render failed");
        let bytes = std::fs::read(&path).expect("png missing");
        assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "not a PNG");
        let img = image::load_from_memory(&bytes).expect("PNG does not decode");
        // Default scale 1.0 renders at exact stage size.
        assert_eq!(img.width(), core.project.stage[0] as u32);
        assert_eq!(img.height(), core.project.stage[1] as u32);
        assert_eq!(out["width"], core.project.stage[0] as u32);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_frame_base64_decodes_to_stage_size_png() {
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let out = call("render_frame_base64", &mut core, json!({ "frame": 0 }))
            .expect("render_frame_base64 failed");
        let bytes = base64_decode(out["png_base64"].as_str().expect("no png_base64"));
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "base64 payload is not a PNG");
        let img = image::load_from_memory(&bytes).expect("PNG does not decode");
        assert_eq!(img.width(), core.project.stage[0] as u32);
        assert_eq!(img.height(), core.project.stage[1] as u32);
        assert_eq!(out["width"], core.project.stage[0] as u32);
        assert_eq!(out["height"], core.project.stage[1] as u32);
    }

    #[test]
    fn render_contact_sheet_writes_a_png_grid() {
        let dir = temp_dir("contact-sheet");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let total = core.project.frames.len();
        assert!(total >= 2, "demo project needs at least 2 frames");

        // Explicit frames, one out of range: it clamps with a note.
        let path = dir.join("sheet.png");
        let out = call(
            "render_contact_sheet",
            &mut core,
            json!({
                "path": path.to_string_lossy(),
                "frames": [0, total - 1, 9999],
                "cols": 2,
                "thumb_width": 48,
            }),
        )
        .expect("render_contact_sheet failed");
        assert_eq!(out["path"], path.to_string_lossy().as_ref());
        assert_eq!(out["frames"], json!([0, total - 1, total - 1]));
        assert!(out["note"].as_str().is_some_and(|n| n.contains("clamped")));
        assert!(path.exists(), "contact sheet PNG missing");
        let img = image::open(&path).expect("contact sheet does not decode");
        assert_eq!(img.width(), out["width"].as_u64().unwrap() as u32);
        assert_eq!(img.height(), out["height"].as_u64().unwrap() as u32);
        // Grid geometry from export::render_contact_sheet: 2 cols,
        // 2 rows, 8px gaps around 48px-wide thumbnails.
        assert_eq!(img.width(), 2 * 48 + 3 * 8);

        // Defaults: every frame (demo has <= 12), no note.
        let path2 = dir.join("sheet-default.png");
        let out = call(
            "render_contact_sheet",
            &mut core,
            json!({ "path": path2.to_string_lossy() }),
        )
        .expect("render_contact_sheet with defaults failed");
        assert_eq!(out["frames"].as_array().unwrap().len(), total);
        assert!(out.get("note").is_none());
        assert!(path2.exists());

        // Bad arguments error instead of panicking.
        assert!(call(
            "render_contact_sheet",
            &mut core,
            json!({ "path": path.to_string_lossy(), "cols": 0 }),
        )
        .is_err());
        assert!(call(
            "render_contact_sheet",
            &mut core,
            json!({ "path": path.to_string_lossy(), "thumb_width": 8 }),
        )
        .is_err());
        assert!(call(
            "render_contact_sheet",
            &mut core,
            json!({ "path": path.to_string_lossy(), "frames": [] }),
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_piv5_roundtrips_through_importer() {
        let dir = temp_dir("piv5");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("out.piv");
        call(
            "export_file",
            &mut core,
            json!({ "path": path.to_string_lossy(), "format": "piv5" }),
        )
        .expect("export failed");
        let bytes = std::fs::read(&path).expect("piv missing");
        let reimported =
            crate::pivot::import_piv(&bytes).expect("exported .piv does not re-import");
        assert!(!reimported.frames.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_piv2_piv4_roundtrip_through_importer() {
        let dir = temp_dir("piv24");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        for format in ["piv2", "piv4"] {
            let path = dir.join(format!("out.{format}"));
            call(
                "export_file",
                &mut core,
                json!({ "path": path.to_string_lossy(), "format": format }),
            )
            .unwrap_or_else(|e| panic!("{format} export failed: {e}"));
            let bytes = std::fs::read(&path).expect("piv missing");
            let reimported = crate::pivot::import_piv(&bytes)
                .unwrap_or_else(|e| panic!("exported {format} does not re-import: {e}"));
            assert!(!reimported.frames.is_empty(), "{format} lost its frames");
            assert_eq!(reimported.figures.len(), core.project.figures.len(), "{format}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_nodes_roundtrips_through_importer() {
        let dir = temp_dir("nodes");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("fig.nodes");
        call(
            "export_file",
            &mut core,
            json!({ "path": path.to_string_lossy(), "format": "nodes", "figure": 0 }),
        )
        .expect("nodes export failed");
        let bytes = std::fs::read(&path).expect("nodes file missing");
        let (def, _color) = crate::nodes::import_nodes_figure(&bytes)
            .expect("exported .nodes does not re-import");
        assert_eq!(def.nodes.len(), core.project.figures[0].def.nodes.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_stknds_roundtrips_through_importer() {
        let dir = temp_dir("stknds");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("out.stknds");
        call(
            "export_file",
            &mut core,
            json!({ "path": path.to_string_lossy(), "format": "stknds" }),
        )
        .expect("stknds export failed");
        let bytes = std::fs::read(&path).expect("stknds file missing");
        let reimported = crate::nodes_anim::import_nodes_animation(&bytes)
            .expect("exported .stknds does not re-import");
        assert_eq!(reimported.frames.len(), core.project.frames.len());
        assert!(!reimported.figures.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_fixture_through_tool() {
        let mut core = EditorCore::new();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/legacy-v3.piv");
        let out = call("import_file", &mut core, json!({ "path": fixture }))
            .expect("fixture import failed");
        assert_eq!(out["imported_format"], "piv");
        assert!(core.project.frames.len() > 0, "imported project has no frames");

        // A .stk figure adds to the current project instead of
        // replacing it.
        let before = core.project.figures.len();
        let stk = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/figure.stk");
        let out = call("import_file", &mut core, json!({ "path": stk })).expect("stk import failed");
        assert_eq!(out["imported_format"], "stk");
        assert_eq!(core.project.figures.len(), before + 1);
    }

    fn manifest_fixture(name: &str) -> Value {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/manifest.json"
        ))
        .expect("manifest missing");
        let manifest: Value = serde_json::from_str(&text).expect("manifest is not JSON");
        manifest["fixtures"][name].clone()
    }

    /// Wave-7 corpus: the Stick Nodes figure fixture must import to
    /// exactly the values tests/fixtures/manifest.json states.
    #[test]
    fn nodes_fixture_matches_manifest() {
        let entry = manifest_fixture("figure.nodes");
        let mut core = EditorCore::new();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/figure.nodes");
        let out = call("import_file", &mut core, json!({ "path": fixture }))
            .expect("nodes fixture import failed");
        assert_eq!(out["imported_format"], "nodes");
        let p = &core.project;
        assert_eq!(p.frames.len(), entry["frames"].as_u64().unwrap() as usize);
        assert_eq!(p.fps, entry["fps"].as_f64().unwrap() as f32);
        let stated = entry["figures"].as_array().unwrap();
        assert_eq!(p.figures.len(), stated.len());
        let fig = &p.figures[0];
        assert_eq!(fig.name, stated[0]["name"].as_str().unwrap());
        // Definition nodes = stated segments plus the origin node.
        assert_eq!(
            fig.def.nodes.len(),
            stated[0]["segments"].as_u64().unwrap() as usize + 1
        );
        let root = entry["frame0_roots"][0].as_array().unwrap();
        assert_eq!(
            p.frames[0].poses[0].root,
            [root[0].as_f64().unwrap() as f32, root[1].as_f64().unwrap() as f32]
        );
        // The corpus segment itself: length 80 (see manifest notes).
        assert!((fig.def.nodes[2].length - 80.0).abs() < 0.01);
    }

    /// Wave-7 corpus: the Stick Nodes movieclip fixture (built by
    /// the original suite's syntheticClip helper) imports with the
    /// manifest's name/fps/frame/figure values.
    #[test]
    fn nodemc_fixture_matches_manifest() {
        let entry = manifest_fixture("project.nodemc");
        let mut core = EditorCore::new();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/project.nodemc");
        let out = call("import_file", &mut core, json!({ "path": fixture }))
            .expect("nodemc fixture import failed");
        assert_eq!(out["imported_format"], "nodemc");
        let p = &core.project;
        assert_eq!(p.name, "Synthetic clip");
        assert_eq!(p.frames.len(), entry["frames"].as_u64().unwrap() as usize);
        assert_eq!(p.fps, entry["fps"].as_f64().unwrap() as f32);
        assert_eq!(p.figures.len(), entry["figures"].as_array().unwrap().len());
        assert!(p.frames[0].poses.is_empty(), "the clip frame holds no instances");
    }

    #[test]
    fn bad_arguments_error_instead_of_panicking() {
        let mut core = EditorCore::new();
        assert!(call("save_project", &mut core, json!({})).is_err());
        assert!(call("load_project", &mut core, json!({ "path": "/nonexistent/x.json" })).is_err());
        assert!(call("render", &mut core, json!({ "path": "/tmp/x.out", "format": "tiff" })).is_err());
        assert!(call(
            "render",
            &mut core,
            json!({ "path": "/tmp/x.png", "format": "png", "frame": 99999 })
        )
        .is_err());
        assert!(call(
            "export_file",
            &mut core,
            json!({ "path": "/tmp/x.stk", "format": "stk", "figure": 99999 })
        )
        .is_err());
        // The range video formats require integer 'from'/'to'.
        assert!(call(
            "export_file",
            &mut core,
            json!({ "path": "/tmp/x.mp4", "format": "mp4_range" })
        )
        .is_err());
        assert!(call(
            "export_file",
            &mut core,
            json!({ "path": "/tmp/x.webm", "format": "webm_range", "from": 0 })
        )
        .is_err());
        assert!(call(
            "export_file",
            &mut core,
            json!({ "path": "/tmp/x.mp4", "format": "mp4_range", "from": 0.5, "to": 2 })
        )
        .is_err());
    }

    // ---- Wave 11 (A5): bookkeeping parity + atomic save ----

    #[test]
    fn save_project_is_atomic_shaped_and_resets_the_dirty_baseline() {
        let dir = temp_dir("atomic-save");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("scene.styk.json");

        // One counted edit: the session reads dirty.
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        call("save_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("save failed");
        assert_eq!(core.edits_since_autosave(), 0, "a save is the new dirty baseline");
        assert_eq!(core.status, format!("Project saved to {}", path.display()));

        // The file is the complete serialised project, and no
        // temp sibling is left behind by the atomic write.
        let text = std::fs::read_to_string(&path).expect("saved file missing");
        let parsed: Project = serde_json::from_str(&text).expect("saved file does not parse");
        assert_eq!(parsed.name, core.project.name);
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");

        // Saving over an existing file replaces it wholesale.
        core.project.name = "Second Save".to_string();
        call("save_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("second save failed");
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed: Project = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.name, "Second Save");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_project_restarts_the_dirty_baseline() {
        let dir = temp_dir("load-baseline");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let path = dir.join("other.styk.json");
        call("save_project", &mut core, json!({ "path": path.to_string_lossy() })).unwrap();

        // Dirty the session, then load: the opened project
        // must not inherit the previous project's edit count.
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        call("load_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("load failed");
        assert_eq!(core.edits_since_autosave(), 0, "open restarts the dirty baseline");
        assert_eq!(core.status, format!("Opened {}", path.display()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn piv_import_restarts_the_dirty_baseline() {
        let mut core = EditorCore::new();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/legacy-v3.piv");
        call("import_file", &mut core, json!({ "path": fixture })).expect("piv import failed");
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "piv import mirrors import_piv's baseline restart"
        );
    }

    #[test]
    fn nodes_import_is_a_single_undoable_edit_like_the_gui() {
        let mut core = EditorCore::new();
        let frames_before = core.project.frames.len();
        let depth_before = core.undo_depth();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/figure.nodes");
        call("import_file", &mut core, json!({ "path": fixture })).expect("nodes import failed");
        assert_eq!(core.undo_depth(), depth_before + 1, "the import snapshots exactly once");
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "the snapshot counts as an edit, as in the GUI"
        );
        core.undo();
        assert_eq!(
            core.project.frames.len(),
            frames_before,
            "one undo restores the pre-import project"
        );
    }

    #[test]
    fn stk_import_lands_at_the_gui_root_and_is_undoable() {
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        let before = core.project.figures.len();
        let depth_before = core.undo_depth();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/figure.stk");
        call("import_file", &mut core, json!({ "path": fixture })).expect("stk import failed");
        assert_eq!(core.project.figures.len(), before + 1);
        assert_eq!(core.undo_depth(), depth_before + 1, "adding the figure snapshots once");
        // editor.rs import_stk's fixed root, recorded on the
        // added figure's frame-0 pose.
        assert_eq!(core.project.frames[0].poses[before].root, [320.0, 450.0]);
        core.undo();
        assert_eq!(core.project.figures.len(), before, "one undo removes the imported figure");
    }

    // ---- Wave 12 (C6): assets survive io save/load byte-identically ----

    #[test]
    fn save_load_roundtrip_preserves_assets_byte_identically() {
        let dir = temp_dir("assets-roundtrip");
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        // Every possible byte value in the asset payload: any
        // lossy step (a text encoding, a base64 slip, a dropped
        // field) breaks the equality below. A sprite instance
        // and a text instance ride along so the instance tables
        // are pinned by the same round-trip.
        let png_bytes: Vec<u8> = (0u16..=255).map(|b| b as u8).collect();
        core.project.assets.push(crate::model::ImageAsset {
            name: "dot.png".to_string(),
            png: png_bytes.clone(),
        });
        core.project.sprites.push(crate::model::SpriteInstance {
            asset: 0,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 120.0,
                scale: 1.5,
                angle: 30.0,
            }],
        });
        core.project.texts.push(crate::model::TextInstance {
            content: "Assets ride along".to_string(),
            size: 24.0,
            color: 0xFFC65D,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 200.0,
                y: 90.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        let assets_before = core.project.assets.clone();
        let sprites_before = serde_json::to_value(&core.project.sprites).unwrap();
        let texts_before = serde_json::to_value(&core.project.texts).unwrap();

        let path = dir.join("with-assets.styk.json");
        call("save_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("save failed");
        let first_bytes = std::fs::read(&path).expect("saved file missing");
        // The asset bytes are in the file as the embedded table,
        // not dropped or rewritten: the parsed file's asset
        // payload equals the in-memory bytes exactly.
        let parsed: Project =
            serde_json::from_slice(&first_bytes).expect("saved file does not parse");
        assert_eq!(parsed.assets, assets_before);

        // Wipe the session's copies, then load through the tool:
        // the restored project carries the same tables.
        core.project.assets.clear();
        core.project.sprites.clear();
        core.project.texts.clear();
        call("load_project", &mut core, json!({ "path": path.to_string_lossy() }))
            .expect("load failed");
        assert_eq!(core.project.assets.len(), 1);
        assert_eq!(core.project.assets[0].name, "dot.png");
        assert_eq!(core.project.assets[0].png, png_bytes);
        assert_eq!(core.project.assets, assets_before);
        assert_eq!(serde_json::to_value(&core.project.sprites).unwrap(), sprites_before);
        assert_eq!(serde_json::to_value(&core.project.texts).unwrap(), texts_before);

        // Saving the reloaded project reproduces the first
        // save byte-for-byte (the serialiser is deterministic:
        // fixed struct field order, no maps in the model).
        let path2 = dir.join("with-assets-again.styk.json");
        call("save_project", &mut core, json!({ "path": path2.to_string_lossy() }))
            .expect("second save failed");
        let second_bytes = std::fs::read(&path2).expect("second saved file missing");
        assert_eq!(second_bytes, first_bytes, "save/load/save must be byte-identical");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Wave R (R13): frame-range exports ----

    /// A core over the 5-frame demo project, fps pinned to
    /// 24.0 per the wave contract.
    fn range_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        core.project.fps = 24.0;
        core
    }

    #[test]
    fn render_range_from_frame_to_frame_writes_exactly_the_range() {
        let dir = temp_dir("range-framed");
        let mut core = range_core();
        assert_eq!(core.project.frames.len(), 5, "demo fixture frame count");
        let target = dir.join("seq");
        let out = call(
            "render",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "png_sequence", "from_frame": 1, "to_frame": 3 }),
        )
        .expect("ranged render failed");
        // The reply counts the range and echoes it under the
        // spelling the caller used.
        assert_eq!(out["frames"], 3);
        assert_eq!(out["from_frame"], 1);
        assert_eq!(out["to_frame"], 3);
        assert!(out.get("from").is_none(), "legacy echo keys must not appear: {out}");
        // Exactly the three named frames were written.
        let written = std::fs::read_dir(&target).unwrap().count();
        assert_eq!(written, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_range_legacy_spelling_and_no_range_shape_unchanged() {
        let dir = temp_dir("range-legacy");
        let mut core = range_core();
        let target = dir.join("seq");
        let out = call(
            "render",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "png_sequence", "from": 1, "to": 3 }),
        )
        .expect("legacy ranged render failed");
        assert_eq!(out["frames"], 3);
        assert_eq!(out["from"], 1);
        assert_eq!(out["to"], 3);
        assert!(out.get("from_frame").is_none(), "framed echo keys must not appear: {out}");

        // Both spellings given and agreeing: accepted; a
        // framed spelling was used, so the echo is framed.
        let agreed = dir.join("agreed");
        let out = call(
            "render",
            &mut core,
            json!({ "path": agreed.to_string_lossy(), "format": "png_sequence", "from": 1, "from_frame": 1, "to": 3, "to_frame": 3 }),
        )
        .expect("agreeing dual-spelling render failed");
        assert_eq!(out["frames"], 3);
        assert_eq!(out["from_frame"], 1);
        assert_eq!(out["to_frame"], 3);

        // No range at all: the whole timeline, and no echo
        // keys of either spelling (D1's shape discipline).
        let whole = dir.join("whole");
        let out = call(
            "render",
            &mut core,
            json!({ "path": whole.to_string_lossy(), "format": "png_sequence" }),
        )
        .expect("full render failed");
        assert_eq!(out["frames"], 5);
        for key in ["from", "to", "from_frame", "to_frame"] {
            assert!(out.get(key).is_none(), "unexpected echo key {key}: {out}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_range_invalid_ranges_refused_before_any_write() {
        let dir = temp_dir("range-refused");
        let mut core = range_core();
        // (extra args, substring the error must carry)
        let cases: Vec<(Value, &str)> = vec![
            // from after to
            (json!({ "from_frame": 3, "to_frame": 1 }), "must not be less than"),
            // past the end of the 5-frame timeline, either spelling
            (json!({ "from_frame": 0, "to_frame": 5 }), "'to_frame' 5 is out of range (project has 5 frames)"),
            (json!({ "from": 0, "to": 99 }), "'to' 99 is out of range (project has 5 frames)"),
            // negative bound
            (json!({ "from_frame": -1, "to_frame": 2 }), "must be a non-negative integer"),
            // a lone bound of either spelling
            (json!({ "from_frame": 1 }), "must be given together"),
            (json!({ "to": 2 }), "must be given together"),
            // the two spellings disagreeing on one bound
            (json!({ "from": 0, "from_frame": 1, "to_frame": 2 }), "disagree"),
        ];
        for (i, (extra, needle)) in cases.iter().enumerate() {
            let target = dir.join(format!("case{i}"));
            let mut args = json!({ "path": target.to_string_lossy(), "format": "png_sequence" });
            for (k, v) in extra.as_object().unwrap() {
                args[k.as_str()] = v.clone();
            }
            let err = call("render", &mut core, args)
                .expect_err("invalid range must be refused");
            assert!(err.contains(needle), "case {i}: error '{err}' must contain '{needle}'");
            assert!(!target.exists(), "case {i}: a refused range wrote {target:?}");
        }
        // A range on a format that takes none (single frame).
        let target = dir.join("single");
        let err = call(
            "render",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "png", "from_frame": 0, "to_frame": 2 }),
        )
        .expect_err("png takes no range");
        assert!(err.contains("not supported for format 'png'"), "{err}");
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_file_range_bounds_validated_before_any_write() {
        let dir = temp_dir("range-export-file");
        let mut core = range_core();
        let target = dir.join("x.mp4");
        // No bounds at all: the Wave 11 missing-argument voice.
        let err = call(
            "export_file",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "mp4_range" }),
        )
        .expect_err("range formats require their bounds");
        assert!(err.contains("missing required integer argument: from"), "{err}");
        // Past the end under the new spelling: refused, no file.
        let err = call(
            "export_file",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "mp4_range", "from_frame": 0, "to_frame": 5 }),
        )
        .expect_err("past-the-end range must be refused");
        assert!(err.contains("'to_frame' 5 is out of range (project has 5 frames)"), "{err}");
        assert!(!target.exists(), "a refused range wrote {target:?}");
        // from after to under the legacy spelling.
        let err = call(
            "export_file",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "webm_range", "from": 3, "to": 1 }),
        )
        .expect_err("inverted range must be refused");
        assert!(err.contains("must not be less than"), "{err}");
        assert!(!target.exists(), "a refused range wrote {target:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ranged_exports_are_read_only() {
        let dir = temp_dir("range-read-only");
        let mut core = range_core();
        let undo_before = core.undo_depth();
        let edits_before = core.edits_since_autosave();
        let target = dir.join("seq");
        call(
            "render",
            &mut core,
            json!({ "path": target.to_string_lossy(), "format": "png_sequence", "from_frame": 0, "to_frame": 2 }),
        )
        .expect("ranged render failed");
        let refused = dir.join("nope");
        assert!(call(
            "render",
            &mut core,
            json!({ "path": refused.to_string_lossy(), "format": "png_sequence", "from_frame": 0, "to_frame": 9 }),
        )
        .is_err());
        assert_eq!(core.undo_depth(), undo_before, "exports must not spend undo steps");
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "exports must not dirty the project"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
