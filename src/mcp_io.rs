//! MCP render/format tools: saving, loading, importing, exporting
//! and rendering, so an AI agent can finish a job end-to-end
//! through MCP without driving the GUI.
//!
//! All paths are plain filesystem paths the server process can
//! access. Parent directories are created where sensible (file
//! targets; the `jpeg`/`svg` render formats treat `path` as a
//! directory and create it). Handlers never panic: every failure
//! is a descriptive `Err(String)` the agent can act on.

use crate::editor::EditorCore;
use crate::mcp::Tool;
use crate::model::Project;
use image::{ImageBuffer, Rgb};
use serde_json::{json, Value};
use std::io::Cursor;
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

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create directory {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}

/// Replace the session project and reset selection/playhead the
/// same way the editor does when it opens a project file.
fn replace_project(core: &mut EditorCore, project: Project) {
    core.project = project;
    core.current = 0;
    core.playhead = 0.0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
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
    std::fs::write(path, &text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
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
    replace_project(core, project);
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
    match ext.as_str() {
        "piv" => {
            let project = crate::pivot::import_piv(&bytes)?;
            replace_project(core, project);
        }
        "nodemc" | "stknds" => {
            let project = crate::nodes_anim::import_nodes_animation(&bytes)?;
            replace_project(core, project);
        }
        "nodes" => {
            // nodes::import_nodes wraps import_nodes_figure into a
            // one-figure project, exactly as the editor imports it.
            let project = crate::nodes::import_nodes(&bytes)?;
            replace_project(core, project);
        }
        "json" => {
            let text =
                String::from_utf8(bytes).map_err(|_| format!("{path} is not valid UTF-8 JSON"))?;
            let project: Project = serde_json::from_str(&text)
                .map_err(|e| format!("{path} is not a valid project JSON: {e}"))?;
            replace_project(core, project);
        }
        "stk" => {
            // A .stk is a single figure: like the editor, it is
            // added to the current project rather than replacing it.
            let (def, name) = crate::pivot::import_stk(&bytes)?;
            let idx = core.project.figures.len();
            let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
            let label = if name.is_empty() { "Imported figure".to_string() } else { name };
            let root = [core.project.stage[0] / 2.0, core.project.stage[1] / 2.0];
            core.project.add_figure_with(def, &label, color, root);
            core.selected_figure = idx;
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
    std::fs::write(path_ref, &bytes).map_err(|e| format!("could not write {path}: {e}"))?;
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

/// The rasterizer renders at 2x supersampling; scale the result to
/// `w`x`h` (exact stage size when w/h are the stage dimensions).
fn scaled(img: &ImageBuffer<Rgb<u8>, Vec<u8>>, w: u32, h: u32) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    if img.width() == w && img.height() == h {
        img.clone()
    } else {
        image::imageops::resize(img, w.max(1), h.max(1), image::imageops::FilterType::Triangle)
    }
}

/// Render one authored frame at the requested output scale
/// (1.0 = exact stage size; the 2x supersampled render is
/// resampled down, which keeps edges smooth).
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
    let img = crate::export::render_frame_at(p, frame as f32);
    let w = (p.stage[0] as f64 * scale).round().max(1.0) as u32;
    let h = (p.stage[1] as f64 * scale).round().max(1.0) as u32;
    let img = scaled(&img, w, h);
    let (w, h) = (img.width(), img.height());
    Ok((encode_png(&img)?, w, h))
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
    let path_ref = Path::new(&path);
    let frames = match format {
        "png" => {
            let frame = arg_frame(core, args)?;
            let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
            let (bytes, w, h) = render_png_bytes(core, frame, scale)?;
            ensure_parent(path_ref)?;
            std::fs::write(path_ref, &bytes)
                .map_err(|e| format!("could not write {path}: {e}"))?;
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
            crate::export::export_gif(project, path_ref)
        }
        "mp4" => {
            ensure_parent(path_ref)?;
            crate::export::export_mp4(project, path_ref)
        }
        "webm" => {
            ensure_parent(path_ref)?;
            crate::export::export_webm(project, path_ref)
        }
        "apng" => {
            ensure_parent(path_ref)?;
            crate::export::export_apng(project, path_ref)
        }
        "spritesheet" => {
            ensure_parent(path_ref)?;
            crate::export::export_spritesheet(project, path_ref)
        }
        "svg" => {
            // svg::export_svg writes one .svg per authored frame
            // into a directory; `path` names that directory.
            std::fs::create_dir_all(path_ref)
                .map_err(|e| format!("could not create directory {path}: {e}"))?;
            crate::svg::export_svg(project, path_ref)
        }
        "jpeg" => {
            // export_jpeg_sequence writes frame-NNNN.jpg files into
            // a directory (creating it); `path` names that directory.
            crate::export::export_jpeg_sequence(project, path_ref)
        }
        _ => {
            return Err(format!(
                "unknown render format '{format}' \
                 (supported: png, gif, mp4, webm, apng, svg, spritesheet, jpeg)"
            ));
        }
    }
    .map_err(|e| format!("render to {path} failed: {e}"))?;
    let mut out = json!({
        "path": path,
        "format": format,
        "frames": frames,
    });
    if matches!(format, "svg" | "jpeg") {
        out["directory"] = json!(path);
    }
    Ok(out)
}

/// Standard-alphabet base64 with padding (no external crate).
fn base64_encode(data: &[u8]) -> String {
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
    // default, up to 12 evenly spaced frames across the timeline.
    let mut clamped = 0usize;
    let frames: Vec<usize> = match args.get("frames") {
        None | Some(Value::Null) => {
            let n = total.min(12);
            (0..n)
                .map(|i| if n == 1 { 0 } else { i * (total - 1) / (n - 1) })
                .collect()
        }
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
    sheet
        .save(path_ref)
        .map_err(|e| format!("could not write {path}: {e}"))?;
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
            description: "Save the current project as StykAnimate project JSON (.styk.json format) at the given path. Parent directories are created.",
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
            description: "Load a StykAnimate project JSON file, replacing the current session project. Selection and playhead reset to the start. Returns the project summary.",
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
            description: "Import a file by extension: .piv (Pivot animation), .nodemc/.stknds (Stick Nodes animation) and .nodes (Stick Nodes figure) replace the project; .stk (Pivot figure) adds the figure to the current project; .json loads project JSON. Returns the project summary.",
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
            description: "Export the project: 'piv2', 'piv3', 'piv4' or 'piv5' write the whole animation as .piv bytes; 'stk' writes a single figure as Pivot .stk bytes and 'nodes' writes it as a Stick Nodes .nodes figure (both take the 'figure' index, defaulting to the selected figure); 'stknds' writes the whole animation as a Stick Nodes project. Parent directories are created.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination file path" },
                    "format": { "type": "string", "enum": ["piv2", "piv3", "piv4", "piv5", "stk", "nodes", "stknds"] },
                    "figure": { "type": "integer", "minimum": 0, "description": "Figure index for formats 'stk' and 'nodes' (default: selected figure)" }
                },
                "required": ["path", "format"]
            }),
            handler: tool_export_file,
        },
        Tool {
            name: "render",
            description: "Render the animation. Formats: 'png' renders ONE frame (arg 'frame', default current; arg 'scale', default 1.0 = stage size) to a PNG file; 'gif', 'mp4' (needs ffmpeg), 'webm', 'apng' and 'spritesheet' render the whole timeline to the file at 'path'; 'svg' and 'jpeg' write one file per frame into the DIRECTORY at 'path' (created if needed). Parent directories are created for file targets.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Output file path (a directory path for formats svg and jpeg)" },
                    "format": { "type": "string", "enum": ["png", "gif", "mp4", "webm", "apng", "svg", "spritesheet", "jpeg"] },
                    "frame": { "type": "integer", "minimum": 0, "description": "Frame index for format 'png' (default: current frame)" },
                    "scale": { "type": "number", "exclusiveMinimum": 0, "description": "Output scale for format 'png' relative to the stage size (default 1.0)" }
                },
                "required": ["path", "format"]
            }),
            handler: tool_render,
        },
        Tool {
            name: "render_frame_base64",
            description: "Render one frame (default: current) as a PNG at exact stage size and return it base64-encoded, so you can SEE the frame. Returns {frame, width, height, png_base64}.",
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
            description: "Render a contact sheet (grid of frame thumbnails) as a PNG file at 'path'. 'frames' is an optional array of frame indices (default: up to 12 evenly spaced frames; out-of-range indices are clamped and reported in a note). 'cols' is the grid column count (default 4, at least 1); 'thumb_width' is each thumbnail's width in pixels (default 240, 16..=1024). Returns {path, width, height, frames, cols, thumb_width}. Parent directories are created.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination PNG file path" },
                    "frames": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "Frame indices to include (default: up to 12 evenly spaced)" },
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
    }
}
