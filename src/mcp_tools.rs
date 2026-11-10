//! MCP content tools: everything an AI agent needs to BUILD an
//! animation through the MCP server — projects, figures, frames,
//! poses, figure definitions, camera, FX, weather, audio, graph
//! channels and layers.
//!
//! Conventions (shared with the rest of the MCP surface):
//! - Arguments are one JSON object; indices are 0-based.
//! - Angles are in degrees (the model's `Pose` convention).
//! - Colours are `0xRRGGBB` integers.
//! - Every handler validates its arguments and returns
//!   `Err(String)` with a clear message on bad input — never panics.
//! - Every handler returns a small JSON summary of what changed.
//!
//! Where an `EditorCore` op already implements the behaviour
//! (figure builder, add/inbetween/delete frame, undo/redo), the
//! handler delegates to it rather than reimplementing; direct model
//! edits are used only where no core op exists (pose writes, camera
//! keys, FX clips, channels, layer fields).

use crate::editor::EditorCore;
use crate::model::{AudioClip, CameraKey, FigureDef, Frame, Pose, Project, Shape, Stroke};
use crate::vfx::{ClipKind, ParticleClip, Weather, WeatherKind};
use serde_json::{json, Value};

// ---------------------------------------------------------------------
// Argument helpers: every one returns Err(String) on bad input.
// ---------------------------------------------------------------------

fn get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// Wave 11 finiteness rule (B3/B8/B9 finding): a JSON number
/// like 1e999 parses to f64 infinity, and a finite f64 like
/// 1e300 still casts to f32 infinity — both must be refused,
/// or tools store infinite coordinates/geometry. Every
/// numeric helper below applies this check.
fn finite_f32(f: f64, key: &str) -> Result<f32, String> {
    let v = f as f32;
    if f.is_finite() && v.is_finite() {
        Ok(v)
    } else {
        Err(format!("argument '{key}' must be a finite number"))
    }
}

fn req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => finite_f32(f, key),
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => finite_f32(f, key).map(Some),
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Ok(None),
    }
}

fn as_usize(v: &Value, key: &str) -> Result<usize, String> {
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

fn req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match get(args, key) {
        Some(v) => as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

fn opt_bool(args: &Value, key: &str) -> Result<Option<bool>, String> {
    match get(args, key) {
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Ok(None),
    }
}

/// Colours travel as `0xRRGGBB` integers.
fn opt_color(args: &Value, key: &str) -> Result<Option<u32>, String> {
    match get(args, key) {
        Some(v) => {
            let n = as_usize(v, key)?;
            u32::try_from(n)
                .map(Some)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Ok(None),
    }
}

fn req_root(args: &Value, key: &str) -> Result<[f32; 2], String> {
    let v = get(args, key).ok_or_else(|| format!("missing required argument '{key}'"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| format!("argument '{key}' must be an [x, y] array"))?;
    if arr.len() != 2 {
        return Err(format!("argument '{key}' must have exactly 2 elements"));
    }
    let x = match arr[0].as_f64() {
        Some(f) => finite_f32(f, key)?,
        None => return Err(format!("argument '{key}[0]' must be a number")),
    };
    let y = match arr[1].as_f64() {
        Some(f) => finite_f32(f, key)?,
        None => return Err(format!("argument '{key}[1]' must be a number")),
    };
    Ok([x, y])
}

fn opt_root(args: &Value, key: &str) -> Result<Option<[f32; 2]>, String> {
    if get(args, key).is_some() {
        req_root(args, key).map(Some)
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------
// Shared validation / summaries
// ---------------------------------------------------------------------

fn check_figure(core: &EditorCore, fi: usize) -> Result<(), String> {
    if fi < core.project.figures.len() {
        Ok(())
    } else {
        Err(format!(
            "figure index {fi} out of range (project has {} figure(s))",
            core.project.figures.len()
        ))
    }
}

fn check_frame(core: &EditorCore, frame: usize) -> Result<(), String> {
    if frame < core.project.frames.len() {
        Ok(())
    } else {
        Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ))
    }
}

fn check_node(core: &EditorCore, fi: usize, ni: usize) -> Result<(), String> {
    check_figure(core, fi)?;
    let n = core.project.figures[fi].def.nodes.len();
    if ni < n {
        Ok(())
    } else {
        Err(format!("node index {ni} out of range (figure {fi} has {n} node(s))"))
    }
}

fn project_summary(core: &EditorCore) -> Value {
    json!({
        "name": core.project.name,
        "fps": core.project.fps,
        "stage": core.project.stage,
        "figures": core.project.figures.len(),
        "frames": core.project.frames.len(),
        "current_frame": core.current,
    })
}

fn pose_json(pose: &Pose) -> Value {
    json!({ "root": pose.root, "angles": pose.angles })
}

/// The pose of `figure` in `frame`, synthesising the figure's rest
/// pose when the frame's pose vec does not cover the figure (can
/// happen in hand-built projects).
fn read_pose(core: &EditorCore, fi: usize, frame: usize) -> Pose {
    let fig = &core.project.figures[fi];
    match core.project.frames[frame].poses.get(fi) {
        Some(p) => p.clone(),
        None => fig.def.rest_pose([core.project.stage[0] / 2.0, core.project.stage[1] / 2.0]),
    }
}

/// Mutably access the pose of `figure` in `frame`, extending the
/// frame's pose vec with rest poses and padding/truncating the angle
/// vec to the figure's node count, following the model's convention
/// of one angle per node.
fn pose_mut<'a>(core: &'a mut EditorCore, fi: usize, frame: usize) -> &'a mut Pose {
    // Precompute everything that needs immutable access first.
    let fallback_root = [core.project.stage[0] / 2.0, core.project.stage[1] / 2.0];
    let first_root = core.project.frames[frame]
        .poses
        .first()
        .map(|p| p.root)
        .unwrap_or(fallback_root);
    let missing: Vec<Pose> = (core.project.frames[frame].poses.len()..=fi)
        .map(|i| core.project.figures[i].def.rest_pose(first_root))
        .collect();
    let rest_angles: Vec<f32> = core.project.figures[fi]
        .def
        .nodes
        .iter()
        .map(|n| n.rest_angle)
        .collect();
    let frame_obj = &mut core.project.frames[frame];
    frame_obj.poses.extend(missing);
    let pose = &mut frame_obj.poses[fi];
    if pose.angles.len() < rest_angles.len() {
        for i in pose.angles.len()..rest_angles.len() {
            pose.angles.push(rest_angles[i]);
        }
    } else {
        pose.angles.truncate(rest_angles.len());
    }
    pose
}

// ---------------------------------------------------------------------
// Schema helper
// ---------------------------------------------------------------------

fn obj_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

// ---------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------

fn tool_new_project(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let name = opt_str(args, "name")?.unwrap_or("Untitled").to_string();
    let fps = opt_f32(args, "fps")?.unwrap_or(12.0);
    if fps <= 0.0 {
        return Err("argument 'fps' must be positive".into());
    }
    let width = opt_f32(args, "width")?.unwrap_or(960.0);
    let height = opt_f32(args, "height")?.unwrap_or(720.0);
    if width <= 0.0 || height <= 0.0 {
        return Err("arguments 'width'/'height' must be positive".into());
    }
    let def = FigureDef::humanoid();
    let root = [width / 2.0, height * 0.65];
    let pose = def.rest_pose(root);
    core.project = Project {
        name,
        fps,
        stage: [width, height],
        background: 0x232830,
        figures: vec![crate::model::FigureInstance {
            name: "Styk".into(),
            color: 0xE8E8EC,
            def,
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        }],
        frames: vec![Frame { poses: vec![pose], duration: 1.0, easing: None, bends: Vec::new() }],
        camera: Vec::new(),
        tween_playback: false,
        easing: crate::model::Easing::Linear,
        camera_follow: None,
        particles: Vec::new(),
        strokes: Vec::new(),
        audio: Vec::new(),
        clips: Vec::new(),
        fx_clips: Vec::new(),
        assets: Vec::new(),
        channels: Vec::new(),
        sprites: Vec::new(),
        texts: Vec::new(),
        weather: None,
        head_ring_scale: 1.0,
        subtitles: Vec::new(),
        subtitle_style: Default::default(),
        format_version: crate::model::FORMAT_VERSION_CURRENT,
    };
    core.current = 0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
    core.playing = false;
    core.note_project_swapped();
    core.status = "New project (via MCP).".into();
    Ok(json!({ "ok": true, "project": project_summary(core) }))
}

fn tool_add_figure(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let kind = req_str(args, "kind")?;
    match kind {
        "humanoid" => core.add_figure(),
        "quadruped" => core.add_quadruped(),
        "mini" => core.add_simple(),
        other => {
            return Err(format!(
                "unknown figure kind '{other}' (expected \"humanoid\", \"quadruped\" or \"mini\")"
            ))
        }
    }
    let idx = core.project.figures.len() - 1;
    if let Some(name) = opt_str(args, "name")? {
        core.project.figures[idx].name = name.to_string();
    }
    if let Some(color) = opt_color(args, "color")? {
        core.project.figures[idx].color = color;
    }
    let fig = &core.project.figures[idx];
    Ok(json!({
        "ok": true,
        "figure": idx,
        "name": fig.name,
        "color": fig.color,
        "nodes": fig.def.nodes.len(),
    }))
}

fn tool_list_figures(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let figures: Vec<Value> = core
        .project
        .figures
        .iter()
        .enumerate()
        .map(|(i, f)| {
            json!({
                "index": i,
                "name": f.name,
                "color": f.color,
                "hidden": f.hidden,
                "locked": f.locked,
                "opacity": f.opacity,
                "nodes": f.def.nodes.len(),
                "clip": f.clip,
                "solo": core.solo == Some(i),
            })
        })
        .collect();
    Ok(json!({ "figures": figures }))
}

fn tool_figure_set_color(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let color = opt_color(args, "color")?
        .ok_or_else(|| "missing required argument 'color'".to_string())?;
    core.set_figure_color(fi, color);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "color": core.project.figures[fi].color,
    }))
}

fn tool_figure_duplicate(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let Some(new_idx) = core.duplicate_figure(fi) else {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    };
    Ok(json!({
        "ok": true,
        "figure": new_idx,
        "duplicated_from": fi,
        "name": core.project.figures[new_idx].name,
        "figures": core.project.figures.len(),
    }))
}

fn tool_get_project_stats(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    use crate::vfx::WeatherKind;
    let p = &core.project;
    let frames = p.frames.len();
    let duration_secs = if p.fps > 0.0 {
        frames as f32 / p.fps
    } else {
        0.0
    };
    let weather = p.weather.map(|w| match w.kind {
        WeatherKind::Rain => "rain",
        WeatherKind::Snow => "snow",
        WeatherKind::Fireflies => "fireflies",
        WeatherKind::Fog => "fog",
        WeatherKind::Storm => "storm",
    });
    Ok(json!({
        "ok": true,
        "figures": p.figures.len(),
        "frames": frames,
        "fps": p.fps,
        "duration_secs": duration_secs,
        "fx_clips": p.fx_clips.len(),
        "audio_clips": p.audio.len(),
        "sprites": p.sprites.len(),
        "texts": p.texts.len(),
        "weather": weather,
        "clips": p.clips.len(),
        "channels": p.channels.len(),
    }))
}

fn tool_add_frame(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // Wave 11 (B2 I-3): no pre-guard — EditorCore::add_frame
    // has a zero-frame recovery branch (it seeds a frame
    // holding every figure's rest pose), so an empty project
    // is a legal call, not an error.
    core.add_frame();
    Ok(json!({
        "ok": true,
        "frames": core.project.frames.len(),
        "current_frame": core.current,
    }))
}

fn tool_insert_inbetween(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if core.current + 1 >= core.project.frames.len() {
        return Err(
            "insert_inbetween needs a frame after the current one; goto an earlier frame first"
                .into(),
        );
    }
    core.insert_inbetween();
    Ok(json!({
        "ok": true,
        "frames": core.project.frames.len(),
        "current_frame": core.current,
    }))
}

fn tool_delete_frame(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    if core.project.frames.len() <= 1 {
        return Err("cannot delete the only frame".into());
    }
    let prev = core.current;
    core.goto(frame);
    core.delete_frame();
    let new_len = core.project.frames.len();
    let restore = if prev > frame { prev - 1 } else { prev.min(new_len - 1) };
    core.goto(restore);
    Ok(json!({ "ok": true, "deleted": frame, "frames": new_len }))
}

fn tool_set_frame_duration(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let duration = req_f32(args, "duration")?;
    if duration <= 0.0 {
        return Err("argument 'duration' must be positive".into());
    }
    // Wave 13 (B9, dirty census): a timing tweak in the
    // `adjust_frame_duration` family — no undo step, but a
    // real change counts for dirty/autosave via the seam
    // (previously this direct write was invisible to both).
    if core.project.frames[frame].duration != duration {
        core.project.frames[frame].duration = duration;
        core.mark_dirty();
    }
    Ok(json!({ "ok": true, "frame": frame, "duration": duration }))
}

fn tool_set_frame_durations(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let raw = match get(args, "durations") {
        Some(v) => v
            .as_array()
            .ok_or_else(|| "argument 'durations' must be an array of numbers".to_string())?,
        None => return Err("missing required argument 'durations'".into()),
    };
    // Validate everything before mutating, mirroring
    // set_frame_duration's rule (each duration must be positive),
    // so a bad entry never leaves a half-updated timeline.
    let mut durations: Vec<f32> = Vec::with_capacity(raw.len());
    for (i, v) in raw.iter().enumerate() {
        let d = v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("argument 'durations[{i}]' must be a number"))?;
        if d <= 0.0 {
            return Err(format!("argument 'durations[{i}]' must be positive"));
        }
        durations.push(d);
    }
    let updated = durations.len().min(core.project.frames.len());
    // Wave 13 (B9, dirty census): as `set_frame_duration` —
    // one counted edit for the call when any applied value
    // actually changes; identical values mark nothing.
    let mut changed = false;
    for (i, d) in durations.into_iter().take(updated).enumerate() {
        if core.project.frames[i].duration != d {
            core.project.frames[i].duration = d;
            changed = true;
        }
    }
    if changed {
        core.mark_dirty();
    }
    Ok(json!({ "ok": true, "updated": updated }))
}

fn tool_set_project_easing(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let easing = match req_str(args, "easing")? {
        "linear" => crate::model::Easing::Linear,
        "smooth" => crate::model::Easing::Smooth,
        "ease-in" => crate::model::Easing::EaseIn,
        "ease-out" => crate::model::Easing::EaseOut,
        other => {
            return Err(format!(
                "unknown easing '{other}' (expected \"linear\", \"smooth\", \"ease-in\" or \"ease-out\")"
            ))
        }
    };
    core.set_project_easing(easing);
    Ok(json!({
        "ok": true,
        "easing": core.project.easing.label(),
        "status": core.status,
    }))
}

fn tool_get_pose(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let frame = req_usize(args, "frame")?;
    check_figure(core, fi)?;
    check_frame(core, frame)?;
    Ok(pose_json(&read_pose(core, fi, frame)))
}

fn tool_set_pose(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let frame = req_usize(args, "frame")?;
    check_figure(core, fi)?;
    check_frame(core, frame)?;
    let node_count = core.project.figures[fi].def.nodes.len();
    let root = opt_root(args, "root")?;
    // Parse the angles argument up front: either a full array (one
    // angle per node) or an object mapping node index -> degrees.
    let mut angle_updates: Vec<(usize, f32)> = Vec::new();
    if let Some(a) = get(args, "angles") {
        if let Some(arr) = a.as_array() {
            if arr.len() != node_count {
                return Err(format!(
                    "argument 'angles' array must have one entry per node ({node_count}), got {}",
                    arr.len()
                ));
            }
            for (i, v) in arr.iter().enumerate() {
                let deg = match v.as_f64() {
                    Some(f) => finite_f32(f, "angles")?,
                    None => return Err(format!("argument 'angles[{i}]' must be a number")),
                };
                angle_updates.push((i, deg));
            }
        } else if let Some(map) = a.as_object() {
            for (k, v) in map {
                let ni: usize = k
                    .parse()
                    .map_err(|_| format!("argument 'angles' key '{k}' is not a node index"))?;
                if ni >= node_count {
                    return Err(format!(
                        "argument 'angles' key {ni} out of range (figure has {node_count} nodes)"
                    ));
                }
                let deg = match v.as_f64() {
                    Some(f) => finite_f32(f, "angles")?,
                    None => return Err(format!("argument 'angles.{k}' must be a number")),
                };
                angle_updates.push((ni, deg));
            }
        } else {
            return Err(
                "argument 'angles' must be an array or an object of node index -> degrees".into(),
            );
        }
    }
    if root.is_none() && angle_updates.is_empty() {
        return Err("set_pose needs at least one of 'root' or 'angles'".into());
    }
    // Wave 11 undo discipline: one call = one undo step
    // (previously pose authoring was invisible to history —
    // D9's gap). Snapshot after all validation, before the write.
    core.snapshot();
    let changed = angle_updates.len();
    {
        let pose = pose_mut(core, fi, frame);
        if let Some(r) = root {
            pose.root = r;
        }
        for (ni, deg) in angle_updates {
            pose.angles[ni] = deg;
        }
    }
    let pose = read_pose(core, fi, frame);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "angles_changed": changed,
        "pose": pose_json(&pose),
    }))
}

fn parse_shape(v: &Value) -> Result<Shape, String> {
    if let Some(s) = v.as_str() {
        return match s {
            "line" => Ok(Shape::Line),
            "circle" => Ok(Shape::Circle),
            "polygon" => Ok(Shape::Polygon(6)),
            other => Err(format!(
                "unknown shape '{other}' (expected \"line\", \"circle\", \"polygon\" or a side count)"
            )),
        };
    }
    if let Some(n) = v.as_u64() {
        if (3..=12).contains(&n) {
            return Ok(Shape::Polygon(n as u8));
        }
        return Err("polygon side count must be between 3 and 12".into());
    }
    Err("argument 'shape' must be a string or a polygon side count".into())
}

/// The wire name of a node shape (the inverse of
/// [`parse_shape`]): "line", "circle", or "polygon:N".
fn shape_label(s: crate::model::Shape) -> String {
    match s {
        crate::model::Shape::Line => "line".to_string(),
        crate::model::Shape::Circle => "circle".to_string(),
        crate::model::Shape::Polygon(n) => format!("polygon:{n}"),
    }
}

fn tool_set_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    // Wave 11 (C10 G6): clamp to the core adjust ranges, as
    // the segment inspector does — previously any positive
    // length/thickness was stored verbatim.
    let length = opt_f32(args, "length")?.map(|l| l.clamp(4.0, 220.0));
    let thickness = opt_f32(args, "thickness")?.map(|t| t.clamp(1.0, 18.0));
    // "color": null CLEARS the override (the node's colour
    // falls back to the figure colour); an absent key leaves
    // it unchanged — the two must not be conflated.
    enum ColorArg {
        Unchanged,
        Clear,
        Set(u32),
    }
    let color = match args.get("color") {
        None => ColorArg::Unchanged,
        Some(Value::Null) => ColorArg::Clear,
        Some(_) => ColorArg::Set(opt_color(args, "color")?.unwrap_or(0)),
    };
    let shape = match get(args, "shape") {
        Some(v) => Some(parse_shape(v)?),
        None => None,
    };
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    let node = &mut core.project.figures[fi].def.nodes[ni];
    if let Some(l) = length {
        node.length = l;
    }
    if let Some(t) = thickness {
        node.thickness = t;
    }
    match color {
        ColorArg::Unchanged => {}
        ColorArg::Clear => node.color = None,
        ColorArg::Set(c) => node.color = Some(c),
    }
    if let Some(s) = shape {
        node.shape = s;
    }
    let node = &core.project.figures[fi].def.nodes[ni];
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "length": node.length,
        "thickness": node.thickness,
        "color": node.color,
        "shape": shape_label(node.shape),
    }))
}

/// Select (figure, node) so the selected-node style setters
/// (set_node_cap / set_node_color2 / set_node_opacity) act on
/// it — the select-then-delegate pattern add_node and
/// delete_node already use for the builder ops.
fn select_node(core: &mut EditorCore, fi: usize, ni: usize) {
    core.selected_figure = fi;
    core.selected_node = Some(ni);
}

fn tool_set_node_cap(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    let cap = match req_str(args, "cap")? {
        "round" => crate::model::CapKind::Round,
        "square" => crate::model::CapKind::Square,
        "point" => crate::model::CapKind::Point,
        other => {
            return Err(format!(
                "unknown cap '{other}' (expected \"round\", \"square\" or \"point\")"
            ))
        }
    };
    select_node(core, fi, ni);
    core.set_node_cap(cap);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "cap": format!("{:?}", core.project.figures[fi].def.nodes[ni].style.cap).to_lowercase(),
        "status": core.status,
    }))
}

fn tool_set_node_color2(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    // Same convention as fx_set_color2: an absent or null
    // 'color2' clears the gradient back to a solid colour.
    let color2 = opt_color(args, "color2")?;
    select_node(core, fi, ni);
    core.set_node_color2(color2);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "color2": core.project.figures[fi].def.nodes[ni].style.color2,
        "status": core.status,
    }))
}

fn tool_set_node_opacity(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    let opacity = req_f32(args, "opacity")?;
    if opacity < 0.0 {
        return Err("argument 'opacity' must not be negative".into());
    }
    select_node(core, fi, ni);
    core.set_node_opacity(opacity);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        // The op clamps to 0.05..=1.0; read the stored value back.
        "opacity": core.project.figures[fi].def.nodes[ni].style.opacity,
        "status": core.status,
    }))
}

fn tool_add_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let parent = req_usize(args, "parent")?;
    check_node(core, fi, parent)?;
    let length = req_f32(args, "length")?;
    if length < 0.0 {
        return Err("argument 'length' must not be negative".into());
    }
    let thickness = opt_f32(args, "thickness")?;
    if let Some(t) = thickness {
        if t <= 0.0 {
            return Err("argument 'thickness' must be positive".into());
        }
    }
    // Delegate to the figure builder: it appends the segment under
    // the selected node and grows every frame's angle vec.
    core.selected_figure = fi;
    core.selected_node = Some(parent);
    core.add_segment();
    let ni = core.project.figures[fi].def.nodes.len() - 1;
    let node = &mut core.project.figures[fi].def.nodes[ni];
    node.length = length;
    if let Some(t) = thickness {
        node.thickness = t;
    }
    Ok(json!({ "ok": true, "figure": fi, "node": ni, "parent": parent, "length": length }))
}

fn tool_delete_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    let before = core.project.figures[fi].def.nodes.len();
    core.selected_figure = fi;
    core.selected_node = Some(ni);
    core.delete_segment();
    if core.project.figures[fi].def.nodes.len() == before {
        // The builder refused (root, or a node with children) and
        // explained why in the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "deleted": ni,
        "nodes": core.project.figures[fi].def.nodes.len(),
    }))
}

fn tool_set_camera(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let x = req_f32(args, "x")?;
    let y = req_f32(args, "y")?;
    let zoom = req_f32(args, "zoom")?;
    if zoom <= 0.0 {
        return Err("argument 'zoom' must be positive".into());
    }
    let angle = opt_f32(args, "angle")?;
    // Wave 11 (C2 GAP-4): snapshot like every other camera
    // write (previously invisible to undo and the autosave
    // counter), and store the angle WRAPPED to -180..180 —
    // the convention every panel write uses (rotate_camera's
    // norm_deg); evaluation is wrap-safe, so this changes
    // storage only, never the picture.
    core.snapshot();
    core.project.set_camera_key(frame, x, y, zoom);
    if let Some(a) = angle {
        if let Some(k) = core.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = crate::model::norm_deg(a);
        }
    }
    let key: &CameraKey = core
        .project
        .camera
        .iter()
        .find(|k| k.frame == frame)
        .ok_or_else(|| "internal error: camera key missing after upsert".to_string())?;
    Ok(json!({
        "ok": true,
        "frame": frame,
        "camera": { "x": key.x, "y": key.y, "zoom": key.zoom, "angle": key.angle },
    }))
}

fn parse_clip_kind(name: &str) -> Result<ClipKind, String> {
    match name {
        "burst" => Ok(ClipKind::Burst),
        "sparkle" => Ok(ClipKind::Sparkle),
        "smoke" => Ok(ClipKind::Smoke),
        "beam" => Ok(ClipKind::Beam),
        "aura" => Ok(ClipKind::Aura),
        "trail" => Ok(ClipKind::Trail),
        "confetti" => Ok(ClipKind::Confetti),
        "shock_ring" | "shockring" | "shock-ring" => Ok(ClipKind::ShockRing),
        "ember" => Ok(ClipKind::Ember),
        "fountain" => Ok(ClipKind::Fountain),
        "orbit" => Ok(ClipKind::Orbit),
        "lightning" => Ok(ClipKind::Lightning),
        "bubbles" => Ok(ClipKind::Bubbles),
        "leaves" => Ok(ClipKind::Leaves),
        "meteor" => Ok(ClipKind::Meteor),
        "petals" => Ok(ClipKind::Petals),
        "splash" => Ok(ClipKind::Splash),
        "spiral" => Ok(ClipKind::Spiral),
        "fireworks" => Ok(ClipKind::Fireworks),
        "aurora" => Ok(ClipKind::Aurora),
        "comet" => Ok(ClipKind::Comet),
        "galaxy" => Ok(ClipKind::Galaxy),
        "prism" => Ok(ClipKind::Prism),
        "vortex" => Ok(ClipKind::Vortex),
        "glitter" => Ok(ClipKind::Glitter),
        "wave" => Ok(ClipKind::Wave),
        "shard" => Ok(ClipKind::Shard),
        other => Err(format!(
            "unknown fx kind '{other}' (expected burst, sparkle, smoke, beam, aura, trail, confetti, shock_ring, ember, fountain, orbit, lightning, bubbles, leaves, meteor, petals, splash, spiral, fireworks, aurora, comet, galaxy, prism, vortex, glitter, wave or shard)"
        )),
    }
}

fn tool_add_fx_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    let kind = parse_clip_kind(req_str(args, "kind")?)?;
    let start = req_f32(args, "start")?;
    if start < 0.0 {
        return Err("argument 'start' must not be negative".into());
    }
    // Wave 11 (D4): the panel refuses a start at/past the
    // project end (such a clip is never sampled); MCP only
    // checked >= 0 and buried the clip.
    if start >= core.project.frames.len() as f32 {
        return Err(format!(
            "argument 'start' is past the project end ({} frames)",
            core.project.frames.len()
        ));
    }
    let duration = req_f32(args, "duration")?;
    if duration <= 0.0 {
        return Err("argument 'duration' must be positive".into());
    }
    let color = opt_color(args, "color")?.unwrap_or(core.project.figures[fi].color);
    let color2 = opt_color(args, "color2")?;
    let length = opt_f32(args, "length")?.unwrap_or(130.0);
    // One call = one undo step (Wave 11 undo discipline;
    // previously the push was invisible to history — D9).
    core.snapshot();
    // Same construction as the editor's own add_fx_clip op.
    let seed = 11 + core.project.fx_clips.len() as u32 * 7;
    core.project.fx_clips.push(crate::model::FxClipEntry {
        clip: ParticleClip {
            start,
            duration,
            kind,
            color,
            color2,
            seed,
            angle: 0.0,
            length,
        },
        figure: fi,
        node: ni,
        hidden: false,
    });
    Ok(json!({
        "ok": true,
        "fx_index": core.project.fx_clips.len() - 1,
        "kind": format!("{kind:?}"),
        "figure": fi,
        "node": ni,
        "start": start,
        "duration": duration,
        "color2": color2,
    }))
}

fn tool_fx_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let clips: Vec<Value> = crate::fx_edit::list(&core.project)
        .iter()
        .map(|info| {
            json!({
                "index": info.index,
                "figure": info.figure,
                "node": info.node,
                "hidden": info.hidden,
                "kind": info.kind,
                "start": info.start,
                "duration": info.duration,
                "end": info.end,
                "color": info.color,
                "color2": info.color2,
                "seed": info.seed,
                "angle": info.angle,
                "length": info.length,
            })
        })
        .collect();
    Ok(json!({ "ok": true, "count": clips.len(), "clips": clips }))
}

fn tool_fx_move(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let start = req_f32(args, "start")?;
    if start < 0.0 {
        return Err("argument 'start' must not be negative".into());
    }
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    // Wave 11 (C4/D4): the schema promises an ABSOLUTE start,
    // but the old handler routed through the whole-frame
    // delta op, silently snapping fractional targets (±0.5
    // frame error). Set the start exactly instead, mirroring
    // fx_move_clip's snapshot + status voice (its frame_label:
    // whole frames without a fraction, fractional with one
    // decimal). No project-end clamp: past-end parking is
    // deliberate (the panel allows it via drag).
    core.snapshot();
    crate::fx_edit::move_clip(&mut core.project, idx, start);
    let landed = core.project.fx_clips[idx].clip.start;
    let label = if landed.fract() == 0.0 {
        format!("{}", landed as i64)
    } else {
        format!("{landed:.1}")
    };
    core.status = format!("FX clip {} moved to frame {}.", idx + 1, label);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "start": landed,
        "status": core.status,
    }))
}

fn tool_fx_resize(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let duration = req_f32(args, "duration")?;
    if duration <= 0.0 {
        return Err("argument 'duration' must be positive".into());
    }
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    // The op resizes by a delta; the tool's contract stays
    // the absolute duration, read back after the call (the
    // op clamps the result to >= 0.1).
    let current = core.project.fx_clips[idx].clip.duration;
    core.fx_resize_clip(idx, duration - current);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "duration": core.project.fx_clips[idx].clip.duration,
    }))
}

fn tool_fx_duplicate(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    let Some(new_idx) = core.fx_duplicate_clip(idx) else {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    };
    Ok(json!({
        "ok": true,
        "fx_index": new_idx,
        "duplicated_from": idx,
        "start": core.project.fx_clips[new_idx].clip.start,
    }))
}

fn tool_fx_delete(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    core.fx_delete_clip(idx);
    Ok(json!({
        "ok": true,
        "deleted": idx,
        "count": core.project.fx_clips.len(),
    }))
}

fn tool_fx_set_color(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let color = opt_color(args, "color")?
        .ok_or_else(|| "missing required argument 'color'".to_string())?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    core.fx_set_color(idx, color);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "color": core.project.fx_clips[idx].clip.color,
    }))
}

fn tool_fx_set_color2(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    // Same convention as add_fx_clip: an absent or null 'color2'
    // means no gradient end — here it clears the clip back to a
    // solid colour.
    let color2 = opt_color(args, "color2")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    core.fx_set_color2(idx, color2);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "color2": core.project.fx_clips[idx].clip.color2,
    }))
}

fn tool_fx_set_anchor(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no fx clip at index {idx}"));
    }
    core.fx_set_anchor(idx, fi, ni);
    let entry = &core.project.fx_clips[idx];
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "figure": entry.figure,
        "node": entry.node,
    }))
}

fn tool_fx_preset_save(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let name = req_str(args, "name")?;
    if name.is_empty() {
        return Err("argument 'name' must not be empty".into());
    }
    let Some(entry) = core.project.fx_clips.get(idx) else {
        return Err(format!("no fx clip at index {idx}"));
    };
    // Only the ParticleClip is stored: figure/node anchors are
    // project-specific and do not belong in a reusable preset.
    let clip = entry.clip.clone();
    // Wave 11 (B5 S2): report the name actually WRITTEN —
    // the never-overwrite rule suffixes collisions (-2, -3),
    // so the requested name can differ from the preset's.
    let written_path = crate::fx_edit::save_clip_preset(&clip, name).map_err(|e| e.to_string())?;
    let written = written_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "preset": written,
        "path": written_path.to_string_lossy(),
        "kind": format!("{:?}", clip.kind),
    }))
}

fn tool_fx_preset_add(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let name = req_str(args, "name")?;
    let frame = req_f32(args, "frame")?;
    if frame < 0.0 {
        return Err("argument 'frame' must not be negative".into());
    }
    let mut clip = crate::fx_edit::load_clip_preset(name).map_err(|e| e.to_string())?;
    // The preset keeps its kind/duration/colour; only its start is
    // overridden by the requested frame.
    clip.start = frame;
    if core.project.figures.is_empty() {
        return Err("project has no figures to anchor the fx clip".into());
    }
    // Anchored like add_fx_clip anchors its clips, on the selected
    // figure's first node (presets carry no anchor of their own).
    let figure = if core.selected_figure < core.project.figures.len() {
        core.selected_figure
    } else {
        0
    };
    let kind = format!("{:?}", clip.kind);
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    core.project.fx_clips.push(crate::model::FxClipEntry { clip, figure, node: 0, hidden: false });
    Ok(json!({
        "ok": true,
        "fx_index": core.project.fx_clips.len() - 1,
        "preset": name,
        "kind": kind,
        "figure": figure,
        "node": 0,
        "start": frame,
    }))
}

fn tool_set_weather(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let kind = req_str(args, "kind")?;
    if kind == "off" {
        // Wave 13 (B9, dirty census): a project-settings
        // write — no undo step, but a real change counts
        // for dirty/autosave via the seam (previously this
        // direct write was invisible to both).
        if core.project.weather.is_some() {
            core.project.weather = None;
            core.mark_dirty();
        }
        return Ok(json!({ "ok": true, "weather": "off" }));
    }
    let wk = match kind {
        "rain" => WeatherKind::Rain,
        "snow" => WeatherKind::Snow,
        "fireflies" => WeatherKind::Fireflies,
        "fog" => WeatherKind::Fog,
        "storm" => WeatherKind::Storm,
        other => {
            return Err(format!(
                "unknown weather kind '{other}' (expected \"rain\", \"snow\", \"fireflies\", \"fog\", \"storm\" or \"off\")"
            ))
        }
    };
    // Defaults come from the one preset table — the Scene
    // panel's per-kind presets in editor.rs (Focused Wave,
    // C1: this tool used to carry its own table, which had
    // drifted — every non-storm kind defaulted to wind 20
    // and fireflies/fog to amount 90). Explicit arguments
    // still win; the amount clamp is unchanged.
    let preset = crate::editor::eds_kind_preset(wk);
    let amount = opt_f32(args, "amount")?.unwrap_or(preset.amount).clamp(0.0, 200.0);
    let wind = opt_f32(args, "wind")?.unwrap_or(preset.wind);
    // Wave 13 (B9, dirty census): as the "off" branch — a
    // settings write that changes the stored weather marks
    // dirty; re-setting the identical weather marks nothing.
    let new_weather = Weather { kind: wk, amount, wind };
    if core.project.weather != Some(new_weather) {
        core.project.weather = Some(new_weather);
        core.mark_dirty();
    }
    Ok(json!({
        "ok": true,
        "weather": { "kind": kind, "amount": amount, "wind": wind },
    }))
}

fn tool_add_audio_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = req_str(args, "path")?;
    if path.is_empty() {
        return Err("argument 'path' must not be empty".into());
    }
    let start_frame = req_usize(args, "start_frame")?;
    // Wave 11 (B6 L3 / C5 G5 / D5): the panel adds a clip only
    // for a file the picker handed it, and badges a bad file
    // afterwards; MCP used to accept anything, sight unseen.
    // Refuse up front instead: the file must exist and carry
    // a supported extension (the clip-health rule — only WAV
    // is decodable; mirrors audio_ops::aop_is_wav_path), and
    // no clip is created when either check fails. The tool
    // description states this refusal explicitly.
    if std::fs::metadata(path).is_err() {
        return Err(format!("audio file not found: {path}"));
    }
    let is_wav = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    if !is_wav {
        return Err(format!(
            "unsupported audio file (only .wav can be decoded): {path}"
        ));
    }
    // One call = one undo step, and the status speaks in the
    // core add op's voice (editor.rs add_audio_clip).
    core.snapshot();
    core.project.audio.push(AudioClip { path: path.to_string(), start_frame, gain: 1.0 , muted: false});
    core.status = format!("Audio clip added at frame {}: {}", start_frame + 1, path);
    Ok(json!({
        "ok": true,
        "audio_index": core.project.audio.len() - 1,
        "path": path,
        "start_frame": start_frame,
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Audio clip tools (phase E): wrappers over the EditorCore
// audio_ops that the Rune audio panel gained. The *_last DSP
// ops materialise a processed copy of the last clip's file and
// repoint the clip, reporting through `core.status` exactly
// like the panel buttons; move/delete are plain timeline
// edits. Handlers pre-check the clip list so an empty project
// is a clean Err, not just a status line (the fig3 idiom).
// ---------------------------------------------------------------------

/// Index of the last audio clip, or a clean error when the
/// project has none (the *_last ops all act on it).
fn check_last_audio(core: &EditorCore) -> Result<usize, String> {
    if core.project.audio.is_empty() {
        Err("project has no audio clips".into())
    } else {
        Ok(core.project.audio.len() - 1)
    }
}

/// Index of audio clip `index`, or a clean error when the
/// project has no clip there (the pre-check the indexed
/// audio tools share; mirrors tool_audio_delete_clip's range
/// check so an out-of-range index is a clean Err, not just a
/// status line).
fn check_audio_index(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.audio.len() {
        Err(format!("no audio clip at index {idx}"))
    } else {
        Ok(idx)
    }
}

/// Wave 11 (B6 L1): the audio DSP ops report failure through
/// `core.status` ("… failed: …") while changing nothing; the
/// handlers used to return `ok: true` regardless, so a client
/// could not tell a processed clip from a missing file. Any
/// status carrying "failed" is the error it describes.
fn audio_op_reply(core: &EditorCore, idx: usize) -> Result<Value, String> {
    if core.status.contains("failed") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_fade(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let idx = check_last_audio(core)?;
    core.audio_fade_last();
    audio_op_reply(core, idx)
}

fn tool_audio_reverse(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let idx = check_last_audio(core)?;
    core.audio_reverse_last();
    audio_op_reply(core, idx)
}

fn tool_audio_set_gain(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let gain = req_f32(args, "gain")?;
    if gain < 0.0 {
        return Err("argument 'gain' must not be negative".into());
    }
    let idx = check_last_audio(core)?;
    core.audio_set_gain_last(gain);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        // Wave 11 (B6 L2): report the gain read back from the
        // clip after the op's clamp, not the requested value.
        "gain": core.project.audio[idx].gain,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_move_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let start_frame = req_usize(args, "start_frame")?;
    let idx = check_last_audio(core)?;
    // The op moves by a signed delta (clamped at frame 0);
    // the tool's contract stays the absolute start frame, so
    // the delta comes from the clip's current start.
    let current = core.project.audio[idx].start_frame as i64;
    let delta = (start_frame as i64 - current).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    core.audio_move_last(delta);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "start_frame": core.project.audio[idx].start_frame,
        "status": core.status,
    }))
}

fn tool_audio_delete_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    if idx >= core.project.audio.len() {
        return Err(format!("no audio clip at index {idx}"));
    }
    let path = core.project.audio[idx].path.clone();
    core.delete_audio_clip(idx);
    Ok(json!({
        "ok": true,
        "deleted": idx,
        "path": path,
        "count": core.project.audio.len(),
    }))
}

// ---------------------------------------------------------------------
// Indexed audio DSP tools (phase I): MCP parity for the
// Phase H audio_ops primaries, which until now were only
// reachable from the audio panel. Each handler takes the
// clip 'index' (0-based), pre-checks it like
// tool_audio_delete_clip, runs the *_at op and reports
// through `core.status` exactly like the *_last tools
// above.
// ---------------------------------------------------------------------

fn tool_audio_echo(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_echo_at(idx);
    audio_op_reply(core, idx)
}

fn tool_audio_lowpass(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_lowpass_at(idx);
    audio_op_reply(core, idx)
}

fn tool_audio_trim_silence(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_trim_silence_at(idx);
    audio_op_reply(core, idx)
}

fn tool_audio_pitch(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_pitch_at(idx);
    audio_op_reply(core, idx)
}

fn tool_audio_compress(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_compress_at(idx);
    audio_op_reply(core, idx)
}

fn tool_audio_split_silences(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    let added = core.audio_split_silences_at(idx);
    if core.status.contains("failed") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "added": added,
        "count": core.project.audio.len(),
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_set_muted(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    let muted = opt_bool(args, "muted")?
        .ok_or_else(|| "missing required argument 'muted'".to_string())?;
    if !crate::audio_ops::audio_set_muted_at(core, idx, muted) {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "muted": muted,
        "status": core.status,
        "project": project_summary(core),
    }))
}

// ---------------------------------------------------------------------
// Phase J parity tools: MCP surface for the phase's new
// EditorCore ops (FX clip split, particle event duplicate,
// normalise-all and whole-figure mirror). Handlers follow
// the phase I indexed-DSP pattern: arguments are validated
// (and indices pre-checked) before the op runs, and the
// outcome is reported through `core.status` alongside the
// project summary. Where an op can refuse after a valid
// pre-check (it returns false / None), the refusal becomes
// a clean Err carrying the op's status line — the
// tool_fx_duplicate convention.
// ---------------------------------------------------------------------

/// Index of FX clip `index`, or a clean error when the
/// project has no clip there (the pre-check the indexed FX
/// tools share; mirrors check_audio_index so an
/// out-of-range index is a clean Err, not just a status
/// line).
fn check_fx_index(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.fx_clips.len() {
        Err(format!("no FX clip at index {idx}"))
    } else {
        Ok(idx)
    }
}

/// Index of particle event `index`, or a clean error when
/// the project has no event there (mirrors check_fx_index).
fn check_particle_index(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.particles.len() {
        Err(format!("no particle event at index {idx}"))
    } else {
        Ok(idx)
    }
}

fn tool_fx_split_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_fx_index(core, req_usize(args, "index")?)?;
    let at_frame = req_f32(args, "at_frame")?;
    if !core.fx_split_clip(idx, at_frame) {
        // The op refused (e.g. the split point is not
        // inside the clip) and explained via the status
        // line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_particle_duplicate_event(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let idx = check_particle_index(core, req_usize(args, "index")?)?;
    let Some(new_idx) = core.particle_duplicate_event(idx) else {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    };
    Ok(json!({
        "ok": true,
        "particle_index": new_idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

/// Parse a particle kind name for particle_set_kind: the
/// three ParticleKind shapes, case-insensitively (the
/// panel's kind cycler names them Burst / Sparkle /
/// Smoke). Mirrors parse_clip_kind's error shape.
fn parse_particle_kind(name: &str) -> Result<crate::model::ParticleKind, String> {
    use crate::model::ParticleKind;
    match name.to_ascii_lowercase().as_str() {
        "burst" => Ok(ParticleKind::Burst),
        "sparkle" => Ok(ParticleKind::Sparkle),
        "smoke" => Ok(ParticleKind::Smoke),
        other => Err(format!(
            "unknown particle kind '{other}' (expected \"burst\", \"sparkle\" or \"smoke\")"
        )),
    }
}

fn tool_particle_set_kind(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_particle_index(core, req_usize(args, "index")?)?;
    let kind = parse_particle_kind(req_str(args, "kind")?)?;
    // The op returns no bool: the pre-check above is the
    // guard, and the op reports through the status line
    // (the tool_fx_set_kind convention).
    core.particle_set_kind(idx, kind);
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "kind": format!("{kind:?}"),
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_particle_set_color(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_particle_index(core, req_usize(args, "index")?)?;
    let color = opt_color(args, "color")?
        .ok_or_else(|| "missing required argument 'color'".to_string())?;
    core.particle_set_color(idx, color);
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "color": core.project.particles[idx].color,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_normalise_all(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let target_peak = opt_f32(args, "target_peak")?.unwrap_or(0.9);
    // The valid peak range is (0, 1]: nought would silence
    // every clip and anything above 1.0 is not a peak a
    // normalised file can reach. (The `!(x > 0.0)` form
    // also rejects NaN.)
    if !(target_peak > 0.0) || target_peak > 1.0 {
        return Err(format!(
            "argument 'target_peak' must be in the range (0, 1], got {target_peak}"
        ));
    }
    let adjusted = core.normalise_all_clips(target_peak);
    Ok(json!({
        "ok": true,
        "adjusted": adjusted,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_mirror_figure(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    if !core.mirror_figure_all_frames(fi) {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "status": core.status,
        "project": project_summary(core),
    }))
}

// ---------------------------------------------------------------------
// Phase K parity tools: the graph channel pair and the FX
// kind setter — panel capabilities (the graph panel's
// duplicate button, the FX panel's kind cycling) that had
// no MCP surface. The graph pair follows the Phase J
// bool-op convention: a pre-check turns a bad index into
// a clean Err, and the op's false becomes Err(core.status).
// ---------------------------------------------------------------------

/// Index of graph channel `idx`, or a clean error when the
/// project has no channel there (the pre-check the graph
/// channel tools share; mirrors check_fx_index so an
/// out-of-range index is a clean Err, not just a status
/// line).
fn check_graph_channel(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.channels.len() {
        Err(format!("no graph channel at index {idx}"))
    } else {
        Ok(idx)
    }
}

fn tool_duplicate_graph_channel(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let idx = check_graph_channel(core, req_usize(args, "channel")?)?;
    let to_figure = req_usize(args, "to_figure")?;
    if !crate::graph_keys::duplicate_channel_to_figure(core, idx, to_figure) {
        // The op refused (the channel's own figure, a
        // target figure that already has a channel for the
        // same target, a node the target rig lacks, or no
        // such figure) and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_delete_graph_channel(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_graph_channel(core, req_usize(args, "channel")?)?;
    if !crate::graph_keys::delete_channel(core, idx) {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

/// Parse an interpolation mode name for set_key_interp:
/// the three Interp modes graph_paste_keys accepts per
/// key, with the same error wording.
fn parse_interp_name(name: &str) -> Result<crate::graph::Interp, String> {
    use crate::graph::Interp;
    match name {
        "hermite" => Ok(Interp::Hermite),
        "linear" => Ok(Interp::Linear),
        "step" => Ok(Interp::Step),
        other => Err(format!(
            "unknown interp '{other}' (expected \"hermite\", \"linear\" or \"step\")"
        )),
    }
}

fn tool_set_key_interp(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_graph_channel(core, req_usize(args, "channel")?)?;
    let key = req_usize(args, "key")?;
    let interp = parse_interp_name(req_str(args, "interp")?)?;
    if !crate::graph_keys::set_key_interp(core, idx, key, interp) {
        // The op refused (no key at that index on the
        // channel) and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "key": key,
        "interp": interp_name(interp),
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_fx_set_kind(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_fx_index(core, req_usize(args, "index")?)?;
    // The file's one clip-kind parser — the same names
    // add_fx_clip accepts, all 27 kinds; its error names
    // the bad value.
    let kind = parse_clip_kind(req_str(args, "kind")?)?;
    // Unlike its colour siblings the op returns no bool:
    // the pre-check above is the guard, and the op reports
    // the change through the status line.
    core.fx_set_kind(idx, kind);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

// ---------------------------------------------------------------------
// Phase L parity tools: the project settings the panels
// set directly — frame rate and stage size (Phase K) and
// the head-ring scale (Phase L). Each op returns no bool:
// it clamps into its range, always sets the status line,
// and snapshots only on a real change, so the handler
// just reads every argument before the op runs (a bad
// argument never leaves a half-applied change) and
// echoes the resulting value plus the status.
// ---------------------------------------------------------------------

fn tool_set_project_fps(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fps = req_f32(args, "fps")?;
    core.set_project_fps(fps);
    Ok(json!({
        "ok": true,
        "fps": core.project.fps,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_set_stage_size(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let width = req_f32(args, "width")?;
    let height = req_f32(args, "height")?;
    core.set_stage_size(width, height);
    Ok(json!({
        "ok": true,
        "stage": core.project.stage,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_set_head_ring_scale(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let scale = req_f32(args, "scale")?;
    core.set_head_ring_scale(scale);
    Ok(json!({
        "ok": true,
        "head_ring_scale": core.project.head_ring_scale,
        "status": core.status,
        "project": project_summary(core),
    }))
}

/// Parse the graph `target` argument shared by graph_key and the
/// channel transform tools: "root_x"/"root_y" (or "rootx"/"rooty"),
/// "angle:N" / "length:N" / "bend:N" strings, or an object
/// {"angle": node} / {"length": node} / {"bend": node}. Node
/// indices are range-checked against the figure.
fn parse_graph_target(
    core: &EditorCore,
    fi: usize,
    args: &Value,
) -> Result<crate::graph::ChannelTarget, String> {
    use crate::graph::ChannelTarget;
    let target_v = get(args, "target").ok_or("missing required argument 'target'")?;
    if let Some(s) = target_v.as_str() {
        match s {
            "root_x" | "rootx" => return Ok(ChannelTarget::RootX),
            "root_y" | "rooty" => return Ok(ChannelTarget::RootY),
            _ => {}
        }
        for (prefix, kind) in [("angle:", 0), ("length:", 1), ("bend:", 2)] {
            if let Some(rest) = s.strip_prefix(prefix) {
                let ni: usize = rest.parse().map_err(|_| {
                    format!("unknown graph target '{s}' (the {prefix} index must be an integer)")
                })?;
                check_node(core, fi, ni)?;
                return Ok(match kind {
                    0 => ChannelTarget::Angle(ni),
                    1 => ChannelTarget::Length(ni),
                    _ => ChannelTarget::Bend(ni),
                });
            }
        }
        return Err(format!(
            "unknown graph target '{s}' (expected \"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or an object with an angle/length/bend key)"
        ));
    }
    if let Some(map) = target_v.as_object() {
        if let Some(v) = map.get("angle") {
            let ni = as_usize(v, "target.angle")?;
            check_node(core, fi, ni)?;
            return Ok(ChannelTarget::Angle(ni));
        }
        if let Some(v) = map.get("length") {
            let ni = as_usize(v, "target.length")?;
            check_node(core, fi, ni)?;
            return Ok(ChannelTarget::Length(ni));
        }
        if let Some(v) = map.get("bend") {
            let ni = as_usize(v, "target.bend")?;
            check_node(core, fi, ni)?;
            return Ok(ChannelTarget::Bend(ni));
        }
        return Err("argument 'target' object must contain an 'angle', 'length' or 'bend' node index".into());
    }
    Err("argument 'target' must be \"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": node}".into())
}

/// Index of the (figure, target) channel for the channel
/// transform tools; a missing channel is a clean error (these
/// tools never create channels implicitly, unlike graph_key).
fn channel_index(
    core: &EditorCore,
    fi: usize,
    target: crate::graph::ChannelTarget,
) -> Result<usize, String> {
    core.project
        .channels
        .iter()
        .position(|c| c.figure == fi && c.target == target)
        .ok_or_else(|| {
            format!("no graph channel for figure {fi} target {target:?}; add keys with graph_key first")
        })
}

fn tool_graph_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target = parse_graph_target(core, fi, args)?;
    let frame = req_f32(args, "frame")?;
    if frame < 0.0 {
        return Err("argument 'frame' must not be negative".into());
    }
    let value = req_f32(args, "value")?;
    // Wave 11 (B8 4.1 / C3): snapshot before mutating — the
    // channel may be created by this call, so the snapshot
    // must precede channel_mut; previously graph_key was not
    // an undo step at all. Finiteness is enforced by req_f32.
    core.snapshot();
    let ch = crate::graph::channel_mut(&mut core.project.channels, fi, target);
    ch.add_key(frame, value);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "value": value,
        "channel_keys": ch.keys.len(),
        "channels": core.project.channels.len(),
        "project": project_summary(core),
    }))
}

fn tool_layers(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    // Wave 13 (B9, dirty census): the layers family —
    // lock / hide / opacity — has undoability deliberately
    // OFF (the core ops `toggle_figure_lock`,
    // `cycle_figure_opacity` and `step_figure_opacity` set
    // the pattern: no snapshot, the write counts for
    // dirty/autosave via the seam). One counted edit for
    // the call when any project field actually changes;
    // identical values mark nothing, and `solo` is editor
    // session state, never a project write.
    let mut changed = false;
    if let Some(locked) = opt_bool(args, "locked")? {
        if core.project.figures[fi].locked != locked {
            core.project.figures[fi].locked = locked;
            changed = true;
        }
    }
    if let Some(hidden) = opt_bool(args, "hidden")? {
        if core.project.figures[fi].hidden != hidden {
            core.project.figures[fi].hidden = hidden;
            changed = true;
        }
    }
    if let Some(opacity) = opt_f32(args, "opacity")? {
        // layers::set_opacity clamps to the supported 0.05..=1.0 range.
        let before = core.project.figures[fi].opacity;
        crate::layers::set_opacity(&mut core.project, fi, opacity);
        if core.project.figures[fi].opacity != before {
            changed = true;
        }
    }
    if changed {
        core.mark_dirty();
    }
    if let Some(solo) = opt_bool(args, "solo")? {
        if solo {
            core.solo = Some(fi);
        } else if core.solo == Some(fi) {
            core.solo = None;
        }
    }
    let fig = &core.project.figures[fi];
    Ok(json!({
        "ok": true,
        "figure": fi,
        "locked": fig.locked,
        "hidden": fig.hidden,
        "opacity": fig.opacity,
        "solo": core.solo == Some(fi),
    }))
}

fn tool_undo(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.undo();
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_redo(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.redo();
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_mirror_pose(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.mirror_selected_pose();
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_smooth_graph_tangents(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.smooth_graph_tangents();
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_set_clip_loop(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    // Wave 11 (C6): an optional 'mode' sets the loop mode
    // directly (one undo step via EditorCore::set_clip_loop_mode);
    // without it the tool keeps its historical cycle behaviour.
    if let Some(mode_name) = opt_str(args, "mode")? {
        let mode = match mode_name {
            "loop" => crate::model::ClipLoop::Loop,
            "ping-pong" => crate::model::ClipLoop::PingPong,
            "once" => crate::model::ClipLoop::Once,
            other => {
                return Err(format!(
                    "unknown loop mode '{other}' (expected \"loop\", \"ping-pong\" or \"once\")"
                ))
            }
        };
        core.set_clip_loop_mode(index, mode);
    } else {
        core.cycle_clip_loop(index);
    }
    let loop_mode = core.project.clips.get(index).map(|c| match c.loop_mode {
        crate::model::ClipLoop::Loop => "loop",
        crate::model::ClipLoop::PingPong => "ping-pong",
        crate::model::ClipLoop::Once => "once",
    });
    Ok(json!({
        "ok": true,
        "status": core.status,
        "loop_mode": loop_mode,
        "project": project_summary(core),
    }))
}

fn tool_bake_graph(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let from = req_usize(args, "from")?;
    let to = req_usize(args, "to")?;
    core.bake_graph_channels(from, to);
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_graph_tangent_preset(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let preset = match req_str(args, "preset")? {
        "smooth" => crate::graph::TangentPreset::Smooth,
        "ease-in" => crate::graph::TangentPreset::EaseIn,
        "ease-out" => crate::graph::TangentPreset::EaseOut,
        "ease-inout" => crate::graph::TangentPreset::EaseInOut,
        "linear" => crate::graph::TangentPreset::Linear,
        other => {
            return Err(format!(
                "unknown tangent preset '{other}' (expected \"smooth\", \"ease-in\", \"ease-out\", \"ease-inout\" or \"linear\")"
            ))
        }
    };
    core.apply_graph_ease_preset(preset);
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
}

fn tool_mirror_channel_time(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target = parse_graph_target(core, fi, args)?;
    // 'centre' (the brief's spelling; "center" accepted too)
    // defaults to the timeline's middle frame.
    let centre = match opt_f32(args, "centre")? {
        Some(c) => Some(c),
        None => opt_f32(args, "center")?,
    }
    .unwrap_or_else(|| (core.project.frames.len().saturating_sub(1)) as f32 / 2.0);
    let idx = channel_index(core, fi, target)?;
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    crate::graph::mirror_keys_time(&mut ch.keys, centre);
    // Mirroring reverses the key order; restore ascending order
    // (a no-op if the graph function already sorted).
    ch.sort_keys();
    let keys = ch.keys.len();
    core.status = format!("Mirrored graph channel time around frame {centre}.");
    Ok(json!({
        "ok": true,
        "status": core.status,
        "project": project_summary(core),
        "figure": fi,
        "target": format!("{target:?}"),
        "centre": centre,
        "keys": keys,
    }))
}

fn tool_scale_channel_values(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target = parse_graph_target(core, fi, args)?;
    let factor = req_f32(args, "factor")?;
    let pivot = opt_f32(args, "pivot")?.unwrap_or(0.0);
    let idx = channel_index(core, fi, target)?;
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    crate::graph::scale_key_values(&mut ch.keys, factor, pivot);
    let keys = ch.keys.len();
    core.status = format!("Scaled graph channel values by {factor} around {pivot}.");
    Ok(json!({
        "ok": true,
        "status": core.status,
        "project": project_summary(core),
        "figure": fi,
        "target": format!("{target:?}"),
        "factor": factor,
        "pivot": pivot,
        "keys": keys,
    }))
}

// ---------------------------------------------------------------------
// The tool table
// ---------------------------------------------------------------------

/// All content-building tools, in the order agents usually need them.

// ---------------------------------------------------------------------
// Graph key copy/paste (phase F): the graph_keys ops the
// keyframe panel gained in phase E. The MCP Tool handler is a
// bare fn pointer over &mut EditorCore — there is no
// server-side clipboard to keep between calls — so copy
// RETURNS the keys to the client and paste takes them back as
// an explicit 'keys' argument (the same key JSON both ways).
// ---------------------------------------------------------------------

fn interp_name(interp: crate::graph::Interp) -> &'static str {
    match interp {
        crate::graph::Interp::Hermite => "hermite",
        crate::graph::Interp::Linear => "linear",
        crate::graph::Interp::Step => "step",
    }
}

fn key_json(key: &crate::graph::Key) -> Value {
    json!({
        "frame": key.frame,
        "value": key.value,
        "in_tan": key.in_tan,
        "out_tan": key.out_tan,
        "interp": interp_name(key.interp),
    })
}

/// Parse the 'keys' argument of graph_paste_keys: an array of
/// key objects in the shape graph_copy_keys returns. 'frame'
/// and 'value' are required per key; tangents default to 0 and
/// 'interp' to "hermite". Everything is validated up front, so
/// a malformed key never leaves a half-pasted channel.
fn parse_keys(args: &Value) -> Result<Vec<crate::graph::Key>, String> {
    let raw = match get(args, "keys") {
        Some(v) => v
            .as_array()
            .ok_or_else(|| "argument 'keys' must be an array of key objects".to_string())?,
        None => return Err("missing required argument 'keys'".into()),
    };
    if raw.is_empty() {
        return Err("argument 'keys' must contain at least one key".into());
    }
    let mut keys = Vec::with_capacity(raw.len());
    for (i, v) in raw.iter().enumerate() {
        let obj = v
            .as_object()
            .ok_or_else(|| format!("argument 'keys[{i}]' must be a key object"))?;
        let field = |name: &str| -> Result<Option<f32>, String> {
            match obj.get(name).filter(|x| !x.is_null()) {
                Some(x) => match x.as_f64() {
                    // Wave 11 finiteness, as in the shared helpers.
                    Some(f) => finite_f32(f, name).map(Some),
                    None => Err(format!("argument 'keys[{i}].{name}' must be a number")),
                },
                None => Ok(None),
            }
        };
        let frame = field("frame")?
            .ok_or_else(|| format!("argument 'keys[{i}]' is missing 'frame'"))?;
        if frame < 0.0 {
            return Err(format!("argument 'keys[{i}].frame' must not be negative"));
        }
        let value = field("value")?
            .ok_or_else(|| format!("argument 'keys[{i}]' is missing 'value'"))?;
        let interp = match obj.get("interp").filter(|x| !x.is_null()) {
            Some(x) => {
                let name = x
                    .as_str()
                    .ok_or_else(|| format!("argument 'keys[{i}].interp' must be a string"))?;
                match name {
                    "hermite" => crate::graph::Interp::Hermite,
                    "linear" => crate::graph::Interp::Linear,
                    "step" => crate::graph::Interp::Step,
                    other => {
                        return Err(format!(
                            "unknown interp '{other}' in 'keys[{i}]' (expected \"hermite\", \"linear\" or \"step\")"
                        ))
                    }
                }
            }
            None => crate::graph::Interp::Hermite,
        };
        keys.push(crate::graph::Key {
            frame,
            value,
            in_tan: field("in_tan")?.unwrap_or(0.0),
            out_tan: field("out_tan")?.unwrap_or(0.0),
            interp,
        });
    }
    Ok(keys)
}

fn tool_graph_copy_keys(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target = parse_graph_target(core, fi, args)?;
    let idx = channel_index(core, fi, target)?;
    let keys = crate::graph_keys::copy_keys(core, idx)
        .ok_or_else(|| "internal error: channel vanished after lookup".to_string())?;
    Ok(json!({
        "ok": true,
        "figure": fi,
        "target": format!("{target:?}"),
        "keys": keys.iter().map(key_json).collect::<Vec<_>>(),
        "key_count": keys.len(),
    }))
}

fn tool_graph_paste_keys(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target = parse_graph_target(core, fi, args)?;
    let at_frame = req_f32(args, "at_frame")?;
    if at_frame < 0.0 {
        return Err("argument 'at_frame' must not be negative".into());
    }
    let keys = parse_keys(args)?;
    let idx = channel_index(core, fi, target)?;
    if !crate::graph_keys::paste_keys(core, idx, &keys, at_frame) {
        // The op refused and explained via the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "target": format!("{target:?}"),
        "pasted": keys.len(),
        "at_frame": at_frame,
        "channel_keys": core.project.channels[idx].keys.len(),
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Drawing tools (wave 7): strokes are project data
// (Project.strokes), manipulated through draw_ops so MCP, the
// editor and the rasterizer all agree on the width conventions
// (empty `widths` = uniform width, else one width per point).
// ---------------------------------------------------------------------

fn req_points(args: &Value, key: &str) -> Result<Vec<crate::model::V2>, String> {
    let v = get(args, key).ok_or_else(|| format!("missing required argument '{key}'"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| format!("argument '{key}' must be an array of [x, y] points"))?;
    let mut points = Vec::with_capacity(arr.len());
    for (i, p) in arr.iter().enumerate() {
        let pair = p
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| format!("argument '{key}[{i}]' must be an [x, y] pair"))?;
        let x = match pair[0].as_f64() {
            Some(f) => finite_f32(f, key)?,
            None => return Err(format!("argument '{key}[{i}][0]' must be a number")),
        };
        let y = match pair[1].as_f64() {
            Some(f) => finite_f32(f, key)?,
            None => return Err(format!("argument '{key}[{i}][1]' must be a number")),
        };
        points.push([x, y]);
    }
    Ok(points)
}

fn tool_draw_shape(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Geometry comes from paint::shape_points so MCP shapes match
    // the editor's own shape tool point-for-point. The stroke goes
    // on the requested frame (Wave 11: 'frame' added for parity
    // with draw_stroke/draw_polygon — previously the current
    // frame was the only target, D3's footgun), defaulting to
    // the current frame like a stroke drawn by hand.
    let kind = match req_str(args, "kind")? {
        "line" => crate::paint::ShapeKind::Line,
        "rect" => crate::paint::ShapeKind::Rect,
        "ellipse" => crate::paint::ShapeKind::Ellipse,
        other => {
            return Err(format!(
                "unknown shape kind '{other}' (expected \"line\", \"rect\" or \"ellipse\")"
            ))
        }
    };
    let x1 = req_f32(args, "x1")?;
    let y1 = req_f32(args, "y1")?;
    let x2 = req_f32(args, "x2")?;
    let y2 = req_f32(args, "y2")?;
    let frame = match get(args, "frame") {
        Some(v) => as_usize(v, "frame")?,
        None => core.current,
    };
    check_frame(core, frame)?;
    // Same pen defaults as draw_stroke (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    // Wave 11 (C7): the stamp panel's fill/opacity/scatter state
    // was unreachable — the family takes them as optional args.
    let fill = opt_color(args, "fill")?;
    let opacity = opt_f32(args, "opacity")?.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&opacity) {
        return Err("argument 'opacity' must be within 0..=1".into());
    }
    let scatter = opt_f32(args, "scatter")?.unwrap_or(0.0);
    if scatter < 0.0 {
        return Err("argument 'scatter' must be zero or positive".into());
    }
    let points = crate::paint::shape_points(kind, [x1, y1], [x2, y2]);
    let n_points = points.len();
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill,
        spray: None,
        opacity,
        scatter,
    });
    Ok(json!({
        "ok": true,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_draw_polygon(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Geometry comes from paint::regular_polygon_points so MCP
    // polygons match the editor's own stamp tools point-for-point:
    // a closed regular polygon (sides + 1 points), first vertex
    // straight up unless 'rotation' (degrees, Wave 11 — the
    // stamp panel's wheel) turns it.
    let cx = req_f32(args, "cx")?;
    let cy = req_f32(args, "cy")?;
    let radius = req_f32(args, "radius")?;
    if radius <= 0.0 {
        return Err("argument 'radius' must be positive".into());
    }
    let sides = match get(args, "sides") {
        Some(v) => as_usize(v, "sides")?,
        None => 5,
    };
    if sides < 3 {
        return Err("argument 'sides' must be at least 3".into());
    }
    let frame = match get(args, "frame") {
        Some(v) => as_usize(v, "frame")?,
        None => core.current,
    };
    check_frame(core, frame)?;
    let rotation = opt_f32(args, "rotation")?.unwrap_or(0.0);
    // Same pen defaults as draw_shape (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    // Wave 11 (C7): optional fill/opacity/scatter, as draw_shape.
    let fill = opt_color(args, "fill")?;
    let opacity = opt_f32(args, "opacity")?.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&opacity) {
        return Err("argument 'opacity' must be within 0..=1".into());
    }
    let scatter = opt_f32(args, "scatter")?.unwrap_or(0.0);
    if scatter < 0.0 {
        return Err("argument 'scatter' must be zero or positive".into());
    }
    let points = crate::paint::regular_polygon_points([cx, cy], radius, sides, rotation - 90.0);
    let n_points = points.len();
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill,
        spray: None,
        opacity,
        scatter,
    });
    Ok(json!({
        "ok": true,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_draw_star(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Geometry comes from paint::star_points: a star outline —
    // outer radius 'radius', inner radius 26/60 of it (the
    // stamp panel's ratio, Wave 11: MCP previously used 0.45,
    // a visibly different star from the panel's), first point
    // straight up unless 'rotation' (degrees) turns it. The
    // stroke goes on the requested frame (default: current).
    let cx = req_f32(args, "cx")?;
    let cy = req_f32(args, "cy")?;
    let radius = req_f32(args, "radius")?;
    if radius <= 0.0 {
        return Err("argument 'radius' must be positive".into());
    }
    let points_count = match get(args, "points") {
        Some(v) => as_usize(v, "points")?,
        None => 5,
    };
    if points_count < 2 {
        return Err("argument 'points' must be at least 2".into());
    }
    let frame = match get(args, "frame") {
        Some(v) => as_usize(v, "frame")?,
        None => core.current,
    };
    check_frame(core, frame)?;
    let rotation = opt_f32(args, "rotation")?.unwrap_or(0.0);
    // Same pen defaults as draw_shape (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    // Wave 11 (C7): optional fill/opacity/scatter, as draw_shape.
    let fill = opt_color(args, "fill")?;
    let opacity = opt_f32(args, "opacity")?.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&opacity) {
        return Err("argument 'opacity' must be within 0..=1".into());
    }
    let scatter = opt_f32(args, "scatter")?.unwrap_or(0.0);
    if scatter < 0.0 {
        return Err("argument 'scatter' must be zero or positive".into());
    }
    let points = crate::paint::star_points(
        [cx, cy],
        radius,
        radius * (26.0 / 60.0),
        points_count,
        rotation - 90.0,
    );
    let n_points = points.len();
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill,
        spray: None,
        opacity,
        scatter,
    });
    Ok(json!({
        "ok": true,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_smooth_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    // Wave 11 (B3 item 3): snapshot first, as in erase_strokes.
    core.snapshot();
    let smoothed = crate::draw_ops::smooth_frame(&mut core.project, frame, 1);
    Ok(json!({
        "ok": true,
        "frame": frame,
        "smoothed": smoothed,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_erase_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let x = req_f32(args, "x")?;
    let y = req_f32(args, "y")?;
    let radius = req_f32(args, "radius")?;
    if radius <= 0.0 {
        return Err("argument 'radius' must be positive".into());
    }
    // Wave 11 (B3 item 3): the erase op is a pure function
    // over the project, so the undo snapshot lives here —
    // taken FIRST, before the op runs, exactly as B3 specced
    // (an erase that hits nothing still records the step;
    // the state it restores is identical, so undo is a no-op).
    core.snapshot();
    let changed = crate::draw_ops::erase(&mut core.project, frame, [x, y], radius);
    Ok(json!({
        "ok": true,
        "frame": frame,
        "changed": changed,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_simplify_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // 'frame' defaults to the current frame (wave-13 tools follow
    // the draw_shape convention for optional frame arguments).
    let frame = match get(args, "frame") {
        Some(v) => as_usize(v, "frame")?,
        None => core.current,
    };
    check_frame(core, frame)?;
    let epsilon = req_f32(args, "epsilon")?;
    if epsilon <= 0.0 {
        return Err("argument 'epsilon' must be positive".into());
    }
    core.snapshot();
    let reduced = crate::draw_ops::simplify_strokes(&mut core.project, frame, epsilon);
    core.status = format!("Simplified strokes on frame {} ({reduced} reduced).", frame + 1);
    Ok(json!({
        "ok": true,
        "status": core.status,
        "project": project_summary(core),
        "frame": frame,
        "epsilon": epsilon,
        "reduced": reduced,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_delete_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // 'index' is the stroke's position in the project-wide
    // stroke list — the order draw_stroke appends in and the
    // stroke inspector lists (per-stroke delete is the
    // editor's delete_stroke_at op; the frame's other strokes
    // are untouched).
    let idx = req_usize(args, "index")?;
    let Some(stroke) = core.project.strokes.get(idx) else {
        return Err(format!("no stroke at index {idx}"));
    };
    let frame = stroke.frame;
    core.delete_stroke_at(idx);
    Ok(json!({
        "ok": true,
        "deleted": idx,
        "frame": frame,
        "count": core.project.strokes.len(),
        "status": core.status,
    }))
}

fn tool_apply_nib(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let nib = match req_str(args, "nib")? {
        "round" => crate::paint::Nib::Round,
        "square" => crate::paint::Nib::Square,
        "calligraphy" => crate::paint::Nib::Calligraphy,
        other => {
            return Err(format!(
                "unknown nib '{other}' (expected \"round\", \"square\" or \"calligraphy\")"
            ))
        }
    };
    let angle = opt_f32(args, "angle")?.unwrap_or(45.0);
    // Wave 13 (B9, dirty census; §7 C6 refinement): one
    // undo step for the call when the frame carries
    // strokes, like its draw-family siblings (draw_shape,
    // erase_strokes, smooth_strokes snapshot the same way)
    // — previously this direct iter_mut rewrite was
    // invisible to history and the autosave counter alike.
    // A frame with no strokes spends nothing.
    if core.project.strokes.iter().any(|s| s.frame == frame) {
        core.snapshot();
    }
    let mut count = 0usize;
    for stroke in core.project.strokes.iter_mut().filter(|s| s.frame == frame) {
        crate::draw_ops::apply_nib(stroke, nib, angle);
        count += 1;
    }
    Ok(json!({ "ok": true, "frame": frame, "strokes": count }))
}

// ---------------------------------------------------------------------
// 3D session tools (wave 7): thin wrappers over the EditorCore
// fig3 ops. The session state lives in EditorCore.fig3; handlers
// check for it themselves so a missing session is a clean Err,
// not just a status line. Summaries carry the core status text.
// ---------------------------------------------------------------------

fn tool_fig3_enter(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.enter_3d();
    let st = core.fig3.as_ref().expect("enter_3d always starts a session");
    Ok(json!({
        "ok": true,
        "active": true,
        "figure": st.figure.name,
        "joints": st.figure.nodes.len(),
        "status": core.status,
    }))
}

fn fig3_summary(core: &EditorCore) -> Value {
    match &core.fig3 {
        Some(st) => {
            let sel = st.selected.min(st.figure.nodes.len().saturating_sub(1));
            json!({
                "active": true,
                "selected": sel,
                "selected_name": st.figure.nodes.get(sel).map(|n| n.name.clone()),
                "joints": st.figure.nodes.len(),
                "camera": {
                    "yaw": st.camera.yaw,
                    // Wave 11 (B9 D1): the panel displays yaw
                    // wrapped; the raw value keeps accumulating
                    // turns. Report both — 'yaw' stays raw for
                    // compatibility, 'yaw_wrapped' is what the
                    // user sees.
                    "yaw_wrapped": crate::model::norm_deg(st.camera.yaw),
                    "pitch": st.camera.pitch,
                },
                "status": core.status,
            })
        }
        None => json!({ "active": false, "status": core.status }),
    }
}

fn tool_fig3_select(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let joint = req_usize(args, "joint")?;
    let n = match &core.fig3 {
        Some(st) => st.figure.nodes.len(),
        None => return Err("no active 3D session; call fig3_enter first".into()),
    };
    if joint >= n {
        return Err(format!("joint index {joint} out of range (3D figure has {n} joint(s))"));
    }
    let st = core.fig3.as_mut().expect("session checked above");
    st.selected = joint;
    core.status = format!("3D joint: {}.", st.figure.nodes[joint].name);
    Ok(fig3_summary(core))
}

fn tool_fig3_rotate(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    if core.fig3.is_none() {
        return Err("no active 3D session; call fig3_enter first".into());
    }
    let axis = req_usize(args, "axis")?;
    if axis > 2 {
        return Err("argument 'axis' must be 0 (X), 1 (Y) or 2 (Z)".into());
    }
    let degrees = req_f32(args, "degrees")?;
    core.fig3_rotate(axis, degrees);
    let mut out = fig3_summary(core);
    out["axis"] = json!(axis);
    out["degrees"] = json!(degrees);
    Ok(out)
}

fn tool_fig3_orbit(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    if core.fig3.is_none() {
        return Err("no active 3D session; call fig3_enter first".into());
    }
    let yaw = req_f32(args, "yaw")?;
    let pitch = req_f32(args, "pitch")?;
    core.fig3_orbit(yaw, pitch);
    Ok(fig3_summary(core))
}

fn tool_fig3_bake(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if core.fig3.is_none() {
        return Err("no active 3D session; call fig3_enter first".into());
    }
    let before = core.project.figures.len();
    core.fig3_bake();
    if core.project.figures.len() != before + 1 {
        return Err(core.status.clone());
    }
    let idx = core.project.figures.len() - 1;
    Ok(json!({
        "ok": true,
        "figure": idx,
        "name": core.project.figures[idx].name,
        "figures": core.project.figures.len(),
        "status": core.status,
    }))
}

fn tool_fig3_mark_b(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if core.fig3.is_none() {
        return Err("no active 3D session; call fig3_enter first".into());
    }
    core.fig3_mark_pose_b();
    Ok(fig3_summary(core))
}

fn tool_fig3_bake_animation(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    if core.fig3.is_none() {
        return Err("no active 3D session; call fig3_enter first".into());
    }
    let frames = req_usize(args, "frames")?;
    if frames < 2 {
        return Err("argument 'frames' must be at least 2".into());
    }
    // Wave 11 (B9 D3 / E6 residual): the bake allocates one
    // project frame per baked frame; an unbounded count is a
    // memory denial-of-service, not an edit. Cap generously.
    if frames > 10_000 {
        return Err("argument 'frames' must be at most 10000".into());
    }
    let before = core.project.figures.len();
    core.fig3_bake_animation(frames);
    if core.project.figures.len() != before + 1 {
        // Refused (no marked pose B): the core explains via status.
        return Err(core.status.clone());
    }
    let idx = core.project.figures.len() - 1;
    Ok(json!({
        "ok": true,
        "figure": idx,
        "name": core.project.figures[idx].name,
        "frames_baked": frames,
        "project_frames": core.project.frames.len(),
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Movieclip tools (wave 7)
// ---------------------------------------------------------------------

fn tool_clip_create(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    // The wave-7 session op: parameterised, snapshot-covering, and
    // tolerant of short pose vecs (it falls back along the frame
    // chain). None means it refused and explained via the status.
    let Some(ci) = core.create_clip_from_figure(fi) else {
        return Err(core.status.clone());
    };
    let clip = &core.project.clips[ci];
    Ok(json!({
        "ok": true,
        "clip": ci,
        "name": clip.name,
        "frames": clip.frames.len(),
        "figure": fi,
        "loops_on_figure": core.project.figures[fi].clip == Some(ci),
        "status": core.status,
    }))
}

fn tool_clip_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let clips: Vec<Value> = core
        .project
        .clips
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "index": i,
                "name": c.name,
                "frames": c.frames.len(),
                "nodes": c.def.nodes.len(),
                // Wave 14 (C5): per-node segment media, so a
                // clip definition's media is visible in the
                // list reply — clip_get_node_media is the
                // addressed read of the same values.
                "media": c.def.nodes.iter()
                    .map(|n| crate::media::segment_media_json(&n.media))
                    .collect::<Vec<_>>(),
                "loop_mode": match c.loop_mode {
                    crate::model::ClipLoop::Loop => "loop",
                    crate::model::ClipLoop::PingPong => "ping-pong",
                    crate::model::ClipLoop::Once => "once",
                },
                // Wave X (P-1): a figure also uses the clip
                // when one of its state keys names it — the
                // base-link-only filter would report a
                // state-keyed clip as unused.
                "used_by": core
                    .project
                    .figures
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        f.clip == Some(i) || f.states.iter().any(|k| k.clip == i)
                    })
                    .map(|(fi, _)| fi)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let count = clips.len();
    Ok(json!({ "clips": clips, "count": count }))
}

// ---------------------------------------------------------------------
// Figure state tools (Wave X, P-1 / GAP #13): animation-state
// keys on figures — one figure switching clips (idle -> walk
// -> attack) across its timeline. The state tools accept a
// clip by index ("clip") or by name ("clip_name"), GAP #13's
// name-accepting surface. Writes route through the core's
// state/linking ops (X2: set_state_key / remove_state_key /
// assign_clip_to_figure / unlink_figure), so panel and MCP
// author through one path and every mutating tool is
// exactly one undo step.
// ---------------------------------------------------------------------

/// Resolve the clip argument shared by the state tools:
/// `clip_name` (exact match; the first clip with the name
/// wins — `clip_list` shows every index) takes precedence,
/// else the `clip` index, range-checked in the
/// `check_figure` voice.
fn resolve_clip_arg(core: &EditorCore, args: &Value) -> Result<usize, String> {
    if let Some(name) = opt_str(args, "clip_name")? {
        return core
            .project
            .clips
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| format!("no clip named '{name}'"));
    }
    let ci = req_usize(args, "clip")?;
    if ci >= core.project.clips.len() {
        return Err(format!(
            "clip index {ci} out of range (project has {} clip(s))",
            core.project.clips.len()
        ));
    }
    Ok(ci)
}

fn state_key_json(core: &EditorCore, key: &crate::model::StateKey) -> Value {
    json!({
        "frame": key.frame,
        "clip": key.clip,
        "clip_name": core.project.clips.get(key.clip).map(|c| c.name.clone()),
        "offset": key.offset,
        "speed": key.speed,
    })
}

fn tool_figure_list_states(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let fig = &core.project.figures[fi];
    let mut keys = fig.states.clone();
    keys.sort_by_key(|k| k.frame);
    let states: Vec<Value> = keys.iter().map(|k| state_key_json(core, k)).collect();
    Ok(json!({
        "figure": fi,
        "name": fig.name,
        "base_clip": fig.clip,
        "base_clip_name": fig
            .clip
            .and_then(|ci| core.project.clips.get(ci))
            .map(|c| c.name.clone()),
        "states": states,
        "count": fig.states.len(),
    }))
}

fn tool_figure_set_state(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let ci = resolve_clip_arg(core, args)?;
    let offset = match args.get("offset") {
        None => 0,
        Some(v) => as_usize(v, "offset")?,
    };
    let speed = opt_f32(args, "speed")?.unwrap_or(1.0);
    let key = crate::model::StateKey {
        frame,
        clip: ci,
        offset,
        speed,
    };
    if !core.set_state_key(fi, key) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "clip": ci,
        "clip_name": core.project.clips[ci].name,
        "offset": offset,
        "speed": speed,
        "states": core.project.figures[fi].states.len(),
        "status": core.status,
    }))
}

fn tool_figure_clear_state(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let frame = req_usize(args, "frame")?;
    // No check_frame here: a key can outlive its frame when
    // the timeline shrinks, and clearing it must stay
    // possible — the op refuses when no key sits there.
    if !core.remove_state_key(fi, frame) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "states": core.project.figures[fi].states.len(),
        "status": core.status,
    }))
}

fn tool_figure_assign_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let ci = resolve_clip_arg(core, args)?;
    let name = core.project.clips[ci].name.clone();
    // The op treats re-assigning the linked clip as a no-op
    // (false, no status churn) — report it as the success it
    // is rather than as an error carrying a stale status.
    if core.project.figures[fi].clip == Some(ci) {
        return Ok(json!({
            "ok": true,
            "figure": fi,
            "clip": ci,
            "clip_name": name,
            "already_linked": true,
        }));
    }
    if !core.assign_clip_to_figure(fi, ci) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "clip": ci,
        "clip_name": name,
        "status": core.status,
    }))
}

fn tool_figure_unlink_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    // Same no-op shape as assign: unlinking an unlinked
    // figure is a success with nothing to do (the op returns
    // false without touching the status).
    if core.project.figures[fi].clip.is_none() {
        return Ok(json!({
            "ok": true,
            "figure": fi,
            "already_unlinked": true,
            "states_kept": core.project.figures[fi].states.len(),
        }));
    }
    if !core.unlink_figure(fi) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": fi,
        "status": core.status,
    }))
}

fn tool_clip_compatible(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let ci = resolve_clip_arg(core, args)?;
    Ok(json!({
        "compatible": core.clip_compatible(fi, ci),
        "figure": fi,
        "clip": ci,
        "clip_name": core.project.clips[ci].name,
        "figure_nodes": core.project.figures[fi].def.nodes.len(),
        "clip_nodes": core.project.clips[ci].def.nodes.len(),
    }))
}

fn tool_clip_create_range(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let start = req_usize(args, "start")?;
    let end = req_usize(args, "end")?;
    let Some(ci) = core.create_clip_from_figure_range(fi, start, end) else {
        return Err(core.status.clone());
    };
    let clip = &core.project.clips[ci];
    Ok(json!({
        "ok": true,
        "clip": ci,
        "name": clip.name,
        "frames": clip.frames.len(),
        "figure": fi,
        "start": start,
        "end": end,
        "loops_on_figure": core.project.figures[fi].clip == Some(ci),
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Bend tool (wave 7): Frame.bends is per-figure, per-node segment
// bend in radians (empty/short vecs mean straight segments).
// ---------------------------------------------------------------------

fn tool_set_bend(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let frame = req_usize(args, "frame")?;
    let ni = req_usize(args, "node")?;
    check_frame(core, frame)?;
    check_node(core, fi, ni)?;
    // Wave 11 (C10 G9): the origin has no segment to bend —
    // the panel refuses it; previously this was a dead write
    // that returned ok.
    if ni == 0 {
        return Err("The origin has no segment to bend.".into());
    }
    let radians = req_f32(args, "radians")?;
    // The panel clamps bends to ±0.7 rad; clamp and read back
    // what actually stored (previously the input was echoed).
    let radians = radians.clamp(-0.7, 0.7);
    // One call = one undo step (Wave 11 undo discipline).
    core.snapshot();
    let fr = &mut core.project.frames[frame];
    while fr.bends.len() <= fi {
        fr.bends.push(Vec::new());
    }
    let row = &mut fr.bends[fi];
    while row.len() <= ni {
        row.push(0.0);
    }
    row[ni] = radians;
    let stored = core.project.frames[frame].bends[fi][ni];
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "node": ni,
        "radians": stored,
        "degrees": stored.to_degrees(),
    }))
}

// ---------------------------------------------------------------------
// Wave 11 integrator-authored handlers: the gaps the squad
// audits specced but no domain file owned (graph paste by
// index, the movieclip list/session surface, project-file
// tools, project background, node media).
// ---------------------------------------------------------------------

fn tool_graph_paste_keys_at(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // B8 §4.4: the by-index twin of graph_paste_keys — a
    // (figure, target) lookup resolves the FIRST matching
    // channel, so a duplicate channel could never be pasted
    // into by name. This one takes the channel index from
    // graph_list_channels directly.
    let idx = req_usize(args, "channel")?;
    if idx >= core.project.channels.len() {
        return Err(format!("no graph channel at index {idx}"));
    }
    let at_frame = req_f32(args, "at_frame")?;
    if at_frame < 0.0 {
        return Err("argument 'at_frame' must not be negative".into());
    }
    let keys = parse_keys(args)?;
    if !crate::graph_keys::paste_keys(core, idx, &keys, at_frame) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "pasted": keys.len(),
        "at_frame": at_frame,
        "channel_keys": core.project.channels[idx].keys.len(),
        "status": core.status,
    }))
}

fn tool_clip_delete(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    if !core.delete_clip(index) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "clips": core.project.clips.len(),
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_clip_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    let name = req_str(args, "name")?;
    let changed = core.rename_clip(index, name);
    if index >= core.project.clips.len() && !changed {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "name": core.project.clips.get(index).map(|c| c.name.clone()),
        "status": core.status,
    }))
}

fn tool_clip_duplicate(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    let Some(new_index) = core.duplicate_clip(index) else {
        return Err(core.status.clone());
    };
    Ok(json!({
        "ok": true,
        "index": new_index,
        "name": core.project.clips[new_index].name,
        "clips": core.project.clips.len(),
        "status": core.status,
    }))
}

fn tool_clip_move(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    let direction = req_str(args, "direction")?;
    let up = match direction {
        "up" => true,
        "down" => false,
        other => {
            return Err(format!(
                "unknown direction '{other}' (expected \"up\" or \"down\")"
            ))
        }
    };
    if !core.move_clip(index, up) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_clip_insert_figure(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    if !core.insert_clip_figure_at(index) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "figure": core.selected_figure,
        "status": core.status,
        "project": project_summary(core),
    }))
}

/// Shared reply for the clip-session tools: the live session
/// as `get_editor_status` reports it, plus the status line.
fn clip_session_reply(core: &EditorCore) -> Value {
    let session = core.clip_edit.as_ref().map(|s| {
        json!({
            "clip": s.clip,
            "name": s.name,
            "frame": s.frame,
            "frames": core.project.clips.get(s.clip).map(|c| c.frames.len()),
        })
    });
    json!({
        "ok": true,
        "session": session,
        "status": core.status,
    })
}

fn tool_clip_enter(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = req_usize(args, "index")?;
    if !core.enter_clip(index) {
        return Err(core.status.clone());
    }
    Ok(clip_session_reply(core))
}

fn tool_clip_exit(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.exit_clip();
    Ok(clip_session_reply(core))
}

fn tool_clip_goto(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    if core.clip_edit.is_none() {
        return Err("Not editing a movieclip.".into());
    }
    core.clip_goto(frame);
    Ok(clip_session_reply(core))
}

fn tool_clip_add_frame(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if core.clip_edit.is_none() {
        return Err("Not editing a movieclip.".into());
    }
    core.clip_add_frame();
    Ok(clip_session_reply(core))
}

fn tool_clip_delete_frame(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if core.clip_edit.is_none() {
        return Err("Not editing a movieclip.".into());
    }
    let before = core
        .clip_edit
        .as_ref()
        .and_then(|s| core.project.clips.get(s.clip))
        .map(|c| c.frames.len());
    core.clip_delete_frame();
    let after = core
        .clip_edit
        .as_ref()
        .and_then(|s| core.project.clips.get(s.clip))
        .map(|c| c.frames.len());
    if before == after {
        // Refused (the clip's only frame) — the op explained
        // via the status line.
        return Err(core.status.clone());
    }
    Ok(clip_session_reply(core))
}

fn tool_clip_set_angle(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let node = req_usize(args, "node")?;
    let degrees = req_f32(args, "degrees")?;
    if core.clip_edit.is_none() {
        return Err("Not editing a movieclip.".into());
    }
    core.clip_set_angle(node, degrees);
    // The op refuses a bad joint via the status line and is
    // silent on success; read the stored angle back so the
    // reply is truthful either way.
    let session = core.clip_edit.clone();
    let stored = session.as_ref().and_then(|s| {
        core.project
            .clips
            .get(s.clip)?
            .frames
            .get(s.frame)?
            .angles
            .get(node)
            .copied()
    });
    match stored {
        Some(v) if (v - degrees).abs() < 1e-4 => Ok(json!({
            "ok": true,
            "node": node,
            "degrees": v,
            "status": core.status,
        })),
        _ => Err(core.status.clone()),
    }
}

fn tool_clip_set_root(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let root = req_root(args, "root")?;
    if core.clip_edit.is_none() {
        return Err("Not editing a movieclip.".into());
    }
    core.clip_set_root(root);
    Ok(clip_session_reply(core))
}

fn tool_clip_get_pose(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let Some(session) = core.clip_edit.clone() else {
        return Err("Not editing a movieclip.".into());
    };
    let Some(clip) = core.project.clips.get(session.clip) else {
        return Err("the session's clip is gone".into());
    };
    let Some(pose) = clip.frames.get(session.frame) else {
        return Err("the session's frame is out of range".into());
    };
    Ok(json!({
        "ok": true,
        "clip": session.clip,
        "name": session.name,
        "frame": session.frame,
        "frames": clip.frames.len(),
        "root": pose.root,
        "angles": pose.angles,
    }))
}

fn tool_set_project_background(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let color = opt_color(args, "color")?
        .ok_or_else(|| "missing required argument 'color'".to_string())?;
    // One undo step: the background is project data like any
    // other (previously it had no writer on either surface's
    // MCP side and no reader anywhere — E7).
    core.snapshot();
    core.project.background = color;
    Ok(json!({
        "ok": true,
        "background": core.project.background,
        "project": project_summary(core),
    }))
}

fn tool_list_projects(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let _ = core;
    // The Home panel's scan of ./projects, as data (E10 G5).
    let dir = std::path::Path::new("projects");
    let projects: Vec<Value> = crate::projects::scan(dir)
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "path": p.path.to_string_lossy(),
                "figures": p.figures,
                "frames": p.frames,
                "fps": p.fps,
                "modified_unix": p
                    .modified
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            })
        })
        .collect();
    Ok(json!({ "ok": true, "count": projects.len(), "projects": projects }))
}

fn tool_save_project_copy(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // The panel's Save a copy, exactly: the copy's name is
    // DERIVED ("{project name} copy" — project_tools::
    // copy_project_name), never typed, and projects::save_copy
    // claims the target with create-new semantics: an existing
    // copy is an error, never an overwrite, never a suffix
    // walk (D8 GAP 2). The session keeps editing the original.
    let name = format!("{} copy", core.project.name);
    let path = crate::projects::save_copy(core, &name)?;
    // Wave 12 (B3): the copy is a new file in ./projects —
    // refresh the cached selector list so it names the copy
    // too (selection preserved by identity; the session
    // keeps editing the original, and recents stay untouched
    // by design, matching the panel's Save a copy).
    crate::projects::refresh_project_files(core);
    Ok(json!({
        "ok": true,
        "name": name,
        "path": path.to_string_lossy(),
        "status": core.status,
    }))
}

fn tool_duplicate_project(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // Duplicates the CURRENT project's saved file (the name
    // the selector points at), with the -copy / -copy-2
    // suffix walk (projects::duplicate_project, D8 GAP 2).
    let Some(path) = core.project_files.get(core.project_index).cloned() else {
        return Err("no current project file to duplicate".into());
    };
    let new_path = crate::projects::duplicate_project(&path).map_err(|e| e.to_string())?;
    // Wave 12 (B3): refresh the cached selector list for the
    // new file (selection preserved by identity).
    crate::projects::refresh_project_files(core);
    Ok(json!({
        "ok": true,
        "path": new_path.to_string_lossy(),
        "status": core.status,
    }))
}

fn tool_project_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let new_name = req_str(args, "new_name")?;
    if new_name.trim().is_empty() {
        return Err("argument 'new_name' must not be blank".into());
    }
    // Renames the CURRENT project's file (projects::
    // rename_project: moves the file to the new name's slug,
    // refusing a target that exists; the name stored INSIDE
    // the file is projects::rename_project's own contract —
    // unchanged until the next save). The session project's
    // name follows, and the selector is repointed at the new
    // path (E9) — without that, the rename would strand the
    // session's file tracking.
    let Some(path) = core.project_files.get(core.project_index).cloned() else {
        return Err("no current project file to rename".into());
    };
    let new_path = crate::projects::rename_project(&path, new_name).map_err(|e| e.to_string())?;
    // Wave 13 (B9, dirty census — the census's project_rename
    // line, executed here because the session-name write
    // lives in this handler): no undo step — undo would
    // restore the old session name against a file that keeps
    // the new one — but a real change counts for
    // dirty/autosave via the seam (previously this write
    // never counted, the Wave 12 E8 remainder).
    let trimmed = new_name.trim().to_string();
    if core.project.name != trimmed {
        core.project.name = trimmed;
        core.mark_dirty();
    }
    // Wave 12 (B3): refresh BEFORE note_project_loaded — the
    // rescan drops the dead old path from the cached list
    // (it would otherwise linger forever), and the loaded
    // note then selects the new path in the fresh list.
    crate::projects::refresh_project_files(core);
    core.note_project_loaded(&new_path);
    Ok(json!({
        "ok": true,
        "path": new_path.to_string_lossy(),
        "name": core.project.name,
        "status": core.status,
    }))
}

fn tool_attach_image_to_node(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // The core op is selection-scoped and file-picker-shaped:
    // it attaches the first PNG in ./import to the selected
    // joint of the selected figure (steer the selection with
    // set_selection first). Its refusals arrive via the
    // status line; success is the one status that names the
    // attached asset.
    if core.selected_node.is_none() {
        return Err("Select a joint first.".into());
    }
    core.attach_image_to_node();
    if core.status.starts_with("Sprite attached") {
        Ok(json!({
            "ok": true,
            "figure": core.selected_figure,
            "node": core.selected_node,
            "status": core.status,
        }))
    } else {
        Err(core.status.clone())
    }
}

fn tool_clear_node_media(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // Selection-scoped like the op (the segment inspector's
    // Clear media); pre-checked so a missing selection is an
    // error without spending the op's unconditional snapshot.
    if core.selected_node.is_none() {
        return Err("Select a joint first.".into());
    }
    core.clear_node_media();
    Ok(json!({
        "ok": true,
        "figure": core.selected_figure,
        "node": core.selected_node,
        "status": core.status,
    }))
}

/// The stdio refusal for one shell tool (Wave 14): shell
/// tools act on the running GUI, which the stdio server
/// does not have. One stamped fn per shell tool, because
/// the registry's handler type is a plain fn pointer —
/// no closure can capture the tool's name. On the live
/// endpoint these handlers never run: live_mcp's drain
/// peeks each call and routes shell names to the shell
/// queue instead (see `shell_control` in the binary).
/// The wording is the one canonical refusal
/// (docs/wave14/SHELL_CONTROL.md section 5).
macro_rules! shell_unavailable_fn {
    ($fn_name:ident, $tool:literal) => {
        fn $fn_name(_core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
            Err(format!(
                "{} needs the live editor: connect to its live endpoint http://127.0.0.1:32117/mcp instead.",
                $tool
            ))
        }
    };
}

shell_unavailable_fn!(tool_shell_unavailable_set_workspace, "shell_set_workspace");
shell_unavailable_fn!(tool_shell_unavailable_get_state, "shell_get_state");
shell_unavailable_fn!(tool_shell_unavailable_set_onion_depths, "shell_set_onion_depths");
shell_unavailable_fn!(tool_shell_unavailable_view_zoom, "shell_view_zoom");
shell_unavailable_fn!(tool_shell_unavailable_view_fit, "shell_view_fit");
shell_unavailable_fn!(tool_shell_unavailable_view_centre, "shell_view_centre");
shell_unavailable_fn!(tool_shell_unavailable_view_reset, "shell_view_reset");
shell_unavailable_fn!(tool_shell_unavailable_get_view, "shell_get_view");
shell_unavailable_fn!(tool_shell_unavailable_select_figure, "shell_select_figure");
shell_unavailable_fn!(tool_shell_unavailable_select_node, "shell_select_node");
shell_unavailable_fn!(tool_shell_unavailable_select_sprite, "shell_select_sprite");
shell_unavailable_fn!(tool_shell_unavailable_select_fx_clip, "shell_select_fx_clip");
shell_unavailable_fn!(tool_shell_unavailable_select_audio_clip, "shell_select_audio_clip");
shell_unavailable_fn!(tool_shell_unavailable_select_frame, "shell_select_frame");
shell_unavailable_fn!(tool_shell_unavailable_select_stroke, "shell_select_stroke");
shell_unavailable_fn!(tool_shell_unavailable_select_particle, "shell_select_particle");
shell_unavailable_fn!(tool_shell_unavailable_select_clip, "shell_select_clip");
shell_unavailable_fn!(tool_shell_unavailable_select_graph_channel, "shell_select_graph_channel");
shell_unavailable_fn!(tool_shell_unavailable_get_selection, "shell_get_selection");
shell_unavailable_fn!(tool_shell_unavailable_set_theme, "shell_set_theme");
shell_unavailable_fn!(tool_shell_unavailable_get_settings, "shell_get_settings");
shell_unavailable_fn!(tool_shell_unavailable_set_setting, "shell_set_setting");
shell_unavailable_fn!(tool_shell_unavailable_set_section, "shell_set_section");
shell_unavailable_fn!(tool_shell_unavailable_set_column_widths, "shell_set_column_widths");
shell_unavailable_fn!(tool_shell_unavailable_get_layout, "shell_get_layout");
shell_unavailable_fn!(tool_shell_unavailable_set_timeline, "shell_set_timeline");
shell_unavailable_fn!(tool_shell_unavailable_get_recent_colours, "shell_get_recent_colours");
shell_unavailable_fn!(tool_shell_unavailable_list_recents, "shell_list_recents");
shell_unavailable_fn!(tool_shell_unavailable_open_recent, "shell_open_recent");
shell_unavailable_fn!(tool_shell_unavailable_reload_project, "shell_reload_project");
shell_unavailable_fn!(tool_shell_unavailable_recover_snapshot, "shell_recover_snapshot");
shell_unavailable_fn!(tool_shell_unavailable_duplicate_project_file, "shell_duplicate_project_file");
shell_unavailable_fn!(tool_shell_unavailable_delete_project_file, "shell_delete_project_file");
shell_unavailable_fn!(tool_shell_unavailable_set_sidebar, "shell_set_sidebar");

pub fn tools() -> Vec<crate::mcp::Tool> {
    use crate::mcp::Tool;
    vec![
        Tool {
            name: "new_project",
            description: "Replace the session project with a fresh one (a single default humanoid figure, one frame).",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" },
                    "fps": { "type": "number" },
                    "width": { "type": "number", "description": "Stage width in units" },
                    "height": { "type": "number", "description": "Stage height in units" }
                }),
                &[],
            ),
            handler: tool_new_project,
        },
        Tool {
            name: "add_figure",
            description: "Add a figure of the given kind to the project; returns its index.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "enum": ["humanoid", "quadruped", "mini"] },
                    "name": { "type": "string" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["kind"],
            ),
            handler: tool_add_figure,
        },
        Tool {
            name: "list_figures",
            description: "List all figures with index, name, colour, layer state and node count.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_list_figures,
        },
        Tool {
            name: "figure_set_color",
            description: "Set a figure's colour (0xRRGGBB) — the colour its segments draw in unless a segment carries its own colour override. A direct field write like the inspector's colour swatches — not an undo step; a real change marks the project dirty.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["figure", "color"],
            ),
            handler: tool_figure_set_color,
        },
        Tool {
            name: "figure_duplicate",
            description: "Duplicate a figure: the copy (same definition, colour and per-frame poses) is inserted right after the original and becomes the selected figure. Returns the new figure index.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" }
                }),
                &["figure"],
            ),
            handler: tool_figure_duplicate,
        },
        Tool {
            name: "get_project_stats",
            description: "Project summary counts: figures, frames, fps, duration in seconds, FX/audio clips, sprites, texts, weather kind (or null), movieclips and graph channels.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_get_project_stats,
        },
        Tool {
            name: "add_frame",
            description: "Append a copy of the current frame after it and move to it. On a zero-frame project it recovers by creating the first frame instead of failing.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_add_frame,
        },
        Tool {
            name: "insert_inbetween",
            description: "Insert a tween frame halfway between the current frame and the next.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_insert_inbetween,
        },
        Tool {
            name: "delete_frame",
            description: "Delete the frame at the given index.",
            schema: obj_schema(json!({ "frame": { "type": "integer" } }), &["frame"]),
            handler: tool_delete_frame,
        },
        Tool {
            name: "set_frame_duration",
            description: "Set a frame's hold-time multiplier (1.0 = one beat at the project fps). Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "duration": { "type": "number" }
                }),
                &["frame", "duration"],
            ),
            handler: tool_set_frame_duration,
        },
        Tool {
            name: "set_frame_durations",
            description: "Set hold-time multipliers for frames 0..n in one call (same positive-value rule as set_frame_duration; extra entries beyond the frame count are ignored). Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(
                json!({
                    "durations": { "type": "array", "items": { "type": "number" } }
                }),
                &["durations"],
            ),
            handler: tool_set_frame_durations,
        },
        Tool {
            name: "set_project_easing",
            description: "Set the project's default easing for tweened playback: \"linear\", \"smooth\", \"ease-in\" or \"ease-out\" (the easing panel's preset buttons). A project-settings tweak — not an undo step.",
            schema: obj_schema(
                json!({
                    "easing": { "type": "string", "enum": ["linear", "smooth", "ease-in", "ease-out"] }
                }),
                &["easing"],
            ),
            handler: tool_set_project_easing,
        },
        Tool {
            name: "get_pose",
            description: "Read a figure's pose in a frame: {root: [x, y], angles: [degrees per node]}. Angles are the STORED values: parent-relative (each relative to its parent segment) and unnormalised (725 means 725, not 5). For the solved world direction of one segment, use get_node.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" }
                }),
                &["figure", "frame"],
            ),
            handler: tool_get_pose,
        },
        Tool {
            name: "set_pose",
            description: "Write a figure's pose in a frame. 'angles' is either a full array (one angle per node, degrees) or an object mapping node index to degrees; 'root' is [x, y]. Angles are stored parent-relative and verbatim (no wrapping — 725 stays 725). One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" },
                    "root": { "type": "array", "items": { "type": "number" } },
                    "angles": { "description": "Full array of degrees, or {nodeIndex: degrees}" }
                }),
                &["figure", "frame"],
            ),
            handler: tool_set_pose,
        },
        Tool {
            name: "mirror_pose",
            description: "Mirror the selected figure's pose in the current frame (left-right flip around its root). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_mirror_pose,
        },
        Tool {
            name: "mirror_figure",
            description: "Mirror a whole figure left-right in EVERY frame (the all-frames counterpart of mirror_pose): every frame's pose is flipped around the figure's root. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" }
                }),
                &["figure"],
            ),
            handler: tool_mirror_figure,
        },
        Tool {
            name: "set_node",
            description: "Edit a figure definition's segment: length (clamped to 4..=220), thickness (clamped to 1..=18), colour override, or shape (\"line\" | \"circle\" | \"polygon\" | side count 3-12). Pass \"color\": null to CLEAR the colour override (an absent 'color' leaves it unchanged). One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "length": { "type": "number" },
                    "thickness": { "type": "number" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "shape": { "description": "\"line\", \"circle\", \"polygon\" or a side count" }
                }),
                &["figure", "node"],
            ),
            handler: tool_set_node,
        },
        Tool {
            name: "set_node_cap",
            description: "Set a segment's end cap directly: \"round\", \"square\" or \"point\" (the style panel's cap control). One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "cap": { "type": "string", "enum": ["round", "square", "point"] }
                }),
                &["figure", "node", "cap"],
            ),
            handler: tool_set_node_cap,
        },
        Tool {
            name: "set_node_color2",
            description: "Set a segment's gradient end colour (0xRRGGBB; the segment blends its own colour -> color2). Omit 'color2' (or pass null) to clear the gradient back to a solid colour. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "color2": { "type": "integer", "description": "0xRRGGBB gradient end colour" }
                }),
                &["figure", "node"],
            ),
            handler: tool_set_node_color2,
        },
        Tool {
            name: "set_node_opacity",
            description: "Set a segment's opacity (the style panel's opacity stepper). Opacity below 0 is refused; any other value is clamped to 0.05-1.0 and read back as stored. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "opacity": { "type": "number", "description": "Opacity; below 0 refused, otherwise clamped to 0.05-1.0" }
                }),
                &["figure", "node", "opacity"],
            ),
            handler: tool_set_node_opacity,
        },
        Tool {
            name: "add_node",
            description: "Append a segment under the given parent node of a figure; returns the new node index.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "parent": { "type": "integer" },
                    "length": { "type": "number" },
                    "thickness": { "type": "number" }
                }),
                &["figure", "parent", "length"],
            ),
            handler: tool_add_node,
        },
        Tool {
            name: "delete_node",
            description: "Delete a leaf segment of a figure (the root and segments with children are refused).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" }
                }),
                &["figure", "node"],
            ),
            handler: tool_delete_node,
        },
        Tool {
            name: "set_camera",
            description: "Upsert a camera key at a frame: centre (x, y), zoom factor (1.0 = neutral, must be positive), optional rotation angle in degrees — stored wrapped to -180..180, the panel's convention. One undo step.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "zoom": { "type": "number" },
                    "angle": { "type": "number" }
                }),
                &["frame", "x", "y", "zoom"],
            ),
            handler: tool_set_camera,
        },
        Tool {
            name: "add_fx_clip",
            description: "Add a timed particle FX clip anchored to a figure's node. start/duration are in frames; a start at or past the project's end is refused (park clips with fx_move instead, which deliberately allows past-end starts). 'color2' optionally sets a gradient end colour (particles blend colour -> color2).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "kind": { "type": "string", "enum": ["burst", "sparkle", "smoke", "beam", "aura", "trail", "confetti", "shock_ring", "ember", "fountain", "orbit", "lightning", "bubbles", "leaves", "meteor", "petals", "splash", "spiral", "fireworks", "aurora", "comet", "galaxy", "prism", "vortex", "glitter", "wave", "shard"] },
                    "start": { "type": "number" },
                    "duration": { "type": "number" },
                    "color": { "type": "integer", "description": "0xRRGGBB (default: figure colour)" },
                    "color2": { "type": "integer", "description": "0xRRGGBB gradient end colour (default: none = solid colour)" },
                    "length": { "type": "number", "description": "Beam reach / trail span (default 130)" }
                }),
                &["figure", "node", "kind", "start", "duration"],
            ),
            handler: tool_add_fx_clip,
        },
        Tool {
            name: "fx_list",
            description: "List all timed FX clips: index, figure, node, kind, start, duration, end frame, hidden, color, color2, seed, angle and length.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_fx_list,
        },
        Tool {
            name: "fx_move",
            description: "Move an FX clip to a new start frame (clamped at 0). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "start": { "type": "number" }
                }),
                &["index", "start"],
            ),
            handler: tool_fx_move,
        },
        Tool {
            name: "fx_resize",
            description: "Set an FX clip's duration in frames (the stored duration is floored at 0.1 by the op). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "duration": { "type": "number" }
                }),
                &["index", "duration"],
            ),
            handler: tool_fx_resize,
        },
        Tool {
            name: "fx_duplicate",
            description: "Duplicate an FX clip: the copy is inserted right after it and starts where the original ends. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_fx_duplicate,
        },
        Tool {
            name: "fx_delete",
            description: "Delete an FX clip by index. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_fx_delete,
        },
        Tool {
            name: "fx_split_clip",
            description: "Split an FX clip into two at 'at_frame' (a frame coordinate inside the clip's span): the first part keeps the clip's start and the second starts at the split point, both on the same figure and node. Errors when the split point is not inside the clip. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "at_frame": { "type": "number", "description": "Frame coordinate to split at" }
                }),
                &["index", "at_frame"],
            ),
            handler: tool_fx_split_clip,
        },
        Tool {
            name: "fx_set_color",
            description: "Set an FX clip's particle colour (0xRRGGBB). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["index", "color"],
            ),
            handler: tool_fx_set_color,
        },
        Tool {
            name: "fx_set_kind",
            description: "Set an FX clip's particle kind by name: burst, sparkle, smoke, beam, aura, trail, confetti, shock_ring, ember, fountain, orbit, lightning, bubbles, leaves, meteor, petals, splash, spiral, fireworks, aurora, comet, galaxy, prism, vortex, glitter, wave or shard (the names add_fx_clip accepts). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "kind": { "type": "string", "description": "Particle kind name" }
                }),
                &["index", "kind"],
            ),
            handler: tool_fx_set_kind,
        },
        Tool {
            name: "fx_set_color2",
            description: "Set an FX clip's gradient end colour (0xRRGGBB; particles blend colour -> color2). Omit 'color2' (or pass null) to clear the gradient back to a solid colour. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "color2": { "type": "integer", "description": "0xRRGGBB gradient end colour" }
                }),
                &["index"],
            ),
            handler: tool_fx_set_color2,
        },
        Tool {
            name: "fx_set_anchor",
            description: "Re-anchor an FX clip to a figure's node — the joint its particles are emitted from. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" }
                }),
                &["index", "figure", "node"],
            ),
            handler: tool_fx_set_anchor,
        },
        Tool {
            name: "fx_preset_save",
            description: "Save the FX clip at 'index' as a reusable preset under 'name' (kind, duration, colour and motion are stored; the figure/node anchor is not).",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &["index", "name"],
            ),
            handler: tool_fx_preset_save,
        },
        Tool {
            name: "fx_preset_add",
            description: "Add a saved FX preset to the project at the given start frame: the preset's kind/duration/colour are kept, its start is overridden by 'frame', and the clip is anchored to the selected figure's first node. Errors if the preset does not exist.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" },
                    "frame": { "type": "number", "description": "Start frame for the added clip" }
                }),
                &["name", "frame"],
            ),
            handler: tool_fx_preset_add,
        },
        Tool {
            name: "particle_duplicate_event",
            description: "Duplicate a particle event (one of the project's timed particle bursts, listed in the stats' particle count): the copy is INSERTED immediately after the original (at index + 1, not appended to the end), events after it shift up one index, and the copy's index is returned. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_particle_duplicate_event,
        },
        Tool {
            name: "particle_set_kind",
            description: "Set a particle event's kind by name: burst, sparkle or smoke (case-insensitive; the shapes the Particles panel cycles). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "kind": { "type": "string", "description": "Particle kind name" }
                }),
                &["index", "kind"],
            ),
            handler: tool_particle_set_kind,
        },
        Tool {
            name: "particle_set_color",
            description: "Set a particle event's colour (0xRRGGBB) — the tint every particle of the event is drawn in. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["index", "color"],
            ),
            handler: tool_particle_set_color,
        },
        Tool {
            name: "set_weather",
            description: "Set continuous weather over the whole timeline, or turn it off. Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "enum": ["rain", "snow", "fireflies", "fog", "storm", "off"] },
                    "amount": { "type": "number", "description": "Particles per frame, 0-200" },
                    "wind": { "type": "number", "description": "Horizontal drift per second" }
                }),
                &["kind"],
            ),
            handler: tool_set_weather,
        },
        Tool {
            name: "add_audio_clip",
            description: "Place an audio file on the timeline starting at a frame. MCP refuses a file that does not exist or is not a WAV — where the GUI panel would add the clip and badge it broken instead. One undo step.",
            schema: obj_schema(
                json!({
                    "path": { "type": "string" },
                    "start_frame": { "type": "integer" }
                }),
                &["path", "start_frame"],
            ),
            handler: tool_add_audio_clip,
        },
        Tool {
            name: "audio_fade",
            description: "Apply a fade-in/fade-out to the LAST audio clip with the editor's fixed fade lengths (0.25 s in, 0.25 s out — not parameterised; use audio_clip_fade for per-clip lengths): a processed copy of its file is written and the clip repointed. One undo step. For a specific clip, prefer the indexed audio_clip_* family.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_audio_fade,
        },
        Tool {
            name: "audio_reverse",
            description: "Reverse the LAST audio clip: a processed copy of its file is written and the clip repointed (the editor's Reverse op). One undo step. For a specific clip, prefer the indexed audio_clip_* family.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_audio_reverse,
        },
        Tool {
            name: "audio_set_gain",
            description: "Apply a gain to the LAST audio clip (the editor's gain op; 1.0 is unity, 0.5 halves the level; the stored gain is clamped and the reply reads it back). One undo step. For a specific clip, prefer the indexed audio_clip_* family.",
            schema: obj_schema(
                json!({
                    "gain": { "type": "number", "description": "Linear gain, 1.0 = unity" }
                }),
                &["gain"],
            ),
            handler: tool_audio_set_gain,
        },
        Tool {
            name: "audio_move_clip",
            description: "Move the LAST audio clip so it starts at 'start_frame' on the timeline.",
            schema: obj_schema(
                json!({
                    "start_frame": { "type": "integer" }
                }),
                &["start_frame"],
            ),
            handler: tool_audio_move_clip,
        },
        Tool {
            name: "audio_delete_clip",
            description: "Delete the audio clip at 'index' from the timeline (the clip's file on disk is not touched).",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_delete_clip,
        },
        Tool {
            name: "audio_echo",
            description: "Apply a feedback echo (0.18s delay, 35% feedback) to the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Echo op). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_echo,
        },
        Tool {
            name: "audio_lowpass",
            description: "Apply a 1200 Hz low-pass filter to the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Lowpass op). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_lowpass,
        },
        Tool {
            name: "audio_trim_silence",
            description: "Trim leading and trailing silence from the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Trim silence op). One undo step when there is silence to trim.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_trim_silence,
        },
        Tool {
            name: "audio_pitch",
            description: "Pitch-shift the audio clip at 'index' up 2 semitones: a processed copy of its file is written and the clip repointed (the editor's Pitch op). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_pitch,
        },
        Tool {
            name: "audio_compress",
            description: "Compress the audio clip at 'index' (threshold 0.5, ratio 4.0): a processed copy of its file is written and the clip repointed (the editor's Compress op). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_compress,
        },
        Tool {
            name: "audio_split_silences",
            description: "Split the audio clip at 'index' wherever silence of 0.25s or more falls: each part is written as its own file and becomes its own clip, and the result reports how many clips were added. One undo step when a split happens.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_audio_split_silences,
        },
        Tool {
            name: "audio_set_muted",
            description: "Mute or unmute the audio clip at 'index': a muted clip keeps its place on the timeline but is left out of playback and exports. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "muted": { "type": "boolean" }
                }),
                &["index", "muted"],
            ),
            handler: tool_audio_set_muted,
        },
        Tool {
            name: "audio_normalise_all",
            description: "Normalise every audio clip on the timeline to the same peak level: each clip's gain is set so its peak reaches 'target_peak' (in the range (0, 1], default 0.9). Reports how many clips were adjusted.",
            schema: obj_schema(
                json!({
                    "target_peak": { "type": "number", "description": "Peak level to normalise to, (0, 1] (default 0.9)" }
                }),
                &[],
            ),
            handler: tool_audio_normalise_all,
        },
        Tool {
            name: "graph_key",
            description: "Add a graph-editor channel key: target is \"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": node}; frame and value are numbers (degrees for angles).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": nodeIndex}" },
                    "frame": { "type": "number" },
                    "value": { "type": "number" }
                }),
                &["figure", "target", "frame", "value"],
            ),
            handler: tool_graph_key,
        },
        Tool {
            name: "smooth_graph_tangents",
            description: "Smooth the tangents of every graph channel key (auto-tangent pass over all channels). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_smooth_graph_tangents,
        },
        Tool {
            name: "bake_graph",
            description: "Bake the graph channels into per-frame keys between frames 'from' and 'to' (inclusive), sampling every channel at each frame in the range. One undo step.",
            schema: obj_schema(
                json!({
                    "from": { "type": "integer" },
                    "to": { "type": "integer" }
                }),
                &["from", "to"],
            ),
            handler: tool_bake_graph,
        },
        Tool {
            name: "graph_tangent_preset",
            description: "Apply an easing preset to the tangents of every key in the SELECTED figure's graph channels (other figures' channels are untouched — graph_tangent_preset_channel addresses any figure's channel by index): \"smooth\", \"ease-in\", \"ease-out\", \"ease-inout\" or \"linear\". One undo step.",
            schema: obj_schema(
                json!({
                    "preset": { "type": "string", "enum": ["smooth", "ease-in", "ease-out", "ease-inout", "linear"] }
                }),
                &["preset"],
            ),
            handler: tool_graph_tangent_preset,
        },
        Tool {
            name: "mirror_channel_time",
            description: "Mirror one graph channel's keys in time around 'centre' (a frame coordinate; default: the timeline's middle frame), so that channel's animation plays backwards. Target addressing matches graph_key (\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": node}). Errors when the channel does not exist. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": nodeIndex}" },
                    "centre": { "type": "number", "description": "Mirror centre frame (default: middle frame)" }
                }),
                &["figure", "target"],
            ),
            handler: tool_mirror_channel_time,
        },
        Tool {
            name: "scale_channel_values",
            description: "Scale one graph channel's key values by 'factor' around 'pivot' (default 0): value' = pivot + (value - pivot) * factor. Target addressing matches graph_key. Errors when the channel does not exist. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": nodeIndex}" },
                    "factor": { "type": "number" },
                    "pivot": { "type": "number", "description": "Value the scaling is centred on (default 0)" }
                }),
                &["figure", "target", "factor"],
            ),
            handler: tool_scale_channel_values,
        },
        Tool {
            name: "graph_copy_keys",
            description: "Copy all keys of one graph channel, returned as key objects ({frame, value, in_tan, out_tan, interp}) to feed to graph_paste_keys. Target addressing matches graph_key. Errors when the channel does not exist.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": nodeIndex}" }
                }),
                &["figure", "target"],
            ),
            handler: tool_graph_copy_keys,
        },
        Tool {
            name: "graph_paste_keys",
            description: "Paste keys into a graph channel: the earliest pasted key lands at 'at_frame' and existing keys within half a frame of a pasted key are replaced. 'keys' is the array graph_copy_keys returns (per key: frame and value required, in_tan/out_tan default 0, interp \"hermite\"/\"linear\"/\"step\" default \"hermite\"). Target addressing matches graph_key; the channel must already exist. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\", \"bend:N\" or {\"angle\": nodeIndex}" },
                    "keys": { "type": "array", "items": { "type": "object" }, "description": "Key objects as returned by graph_copy_keys" },
                    "at_frame": { "type": "number" }
                }),
                &["figure", "target", "keys", "at_frame"],
            ),
            handler: tool_graph_paste_keys,
        },
        Tool {
            name: "duplicate_graph_channel",
            description: "Duplicate a graph channel — addressed by its index in the project's channel list — onto another figure: the clone keeps every key and is appended to the channel list. Errors when the target figure already has a channel for the same target (the clone would be shadowed), is the channel's own figure, or lacks the node an angle/length target names. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer", "description": "Index into the project's graph channel list" },
                    "to_figure": { "type": "integer" }
                }),
                &["channel", "to_figure"],
            ),
            handler: tool_duplicate_graph_channel,
        },
        Tool {
            name: "delete_graph_channel",
            description: "Delete a graph channel by its index in the project's channel list. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer", "description": "Index into the project's graph channel list" }
                }),
                &["channel"],
            ),
            handler: tool_delete_graph_channel,
        },
        Tool {
            name: "set_key_interp",
            description: "Set the interpolation mode of one key on a graph channel — addressed by the channel's index in the project's channel list and the key's index within it: \"hermite\" (smooth tangents), \"linear\" or \"step\". Frame, value and tangents are untouched. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer", "description": "Index into the project's graph channel list" },
                    "key": { "type": "integer", "description": "Index of the key within the channel" },
                    "interp": { "type": "string", "description": "\"hermite\", \"linear\" or \"step\"" }
                }),
                &["channel", "key", "interp"],
            ),
            handler: tool_set_key_interp,
        },
        Tool {
            name: "set_project_fps",
            description: "Set the project's frame rate in frames per second (clamped to 1-60; the Scene panel's frame-rate stepper). One undo step when the rate changes.",
            schema: obj_schema(
                json!({
                    "fps": { "type": "number", "description": "Frames per second" }
                }),
                &["fps"],
            ),
            handler: tool_set_project_fps,
        },
        Tool {
            name: "set_stage_size",
            description: "Set the stage size in pixels (each axis clamped to 64-4096; the Scene panel's stage-size steppers). One undo step when the size changes.",
            schema: obj_schema(
                json!({
                    "width": { "type": "number", "description": "Stage width in pixels" },
                    "height": { "type": "number", "description": "Stage height in pixels" }
                }),
                &["width", "height"],
            ),
            handler: tool_set_stage_size,
        },
        Tool {
            name: "set_head_ring_scale",
            description: "Set the head-ring size as a multiple of the base ring radius (clamped to 0.5-2; applies to every figure's head ring in the stage and all exports). One undo step when the scale changes.",
            schema: obj_schema(
                json!({
                    "scale": { "type": "number", "description": "Head-ring scale multiplier" }
                }),
                &["scale"],
            ),
            handler: tool_set_head_ring_scale,
        },
        Tool {
            name: "layers",
            description: "Apply layer state to a figure: locked, hidden, opacity (0.05-1.0), and solo (true solos this figure in the editor, false clears its solo). Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "locked": { "type": "boolean" },
                    "hidden": { "type": "boolean" },
                    "opacity": { "type": "number" },
                    "solo": { "type": "boolean" }
                }),
                &["figure"],
            ),
            handler: tool_layers,
        },
        Tool {
            name: "undo",
            description: "Undo the last editor operation.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_undo,
        },
        Tool {
            name: "redo",
            description: "Redo the last undone editor operation.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_redo,
        },
        Tool {
            name: "draw_stroke",
            description: "Append a freehand stroke (polyline) to a frame's drawing layer. points is an array of [x, y] stage coordinates; color is 0xRRGGBB (default light grey), width in stage units (default 4, the editor's pen), scatter jitters the stroke's dabs by that radius in stage units (default 0 = clean line), opacity is the stroke's opacity 0-1 (default 1). One undo step.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "scatter": { "type": "number", "description": "dab jitter radius in stage units, >= 0" },
                    "opacity": { "type": "number", "description": "Stroke opacity 0-1 (default 1)" }
                }),
                &["frame", "points"],
            ),
            handler: crate::draw_ops::tool_draw_stroke,
        },
        Tool {
            name: "draw_shape",
            description: "Append a geometric shape as a stroke: kind is \"line\" (2 points), \"rect\" (closed rectangle, 5 points) or \"ellipse\" (25 points), from corner (x1, y1) to corner (x2, y2) in stage coordinates. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill' (0xRRGGBB), 'opacity' (0-1) and 'scatter' (>= 0) mirror the stamp panel's state. One undo step.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "enum": ["line", "rect", "ellipse"] },
                    "x1": { "type": "number" },
                    "y1": { "type": "number" },
                    "x2": { "type": "number" },
                    "y2": { "type": "number" },
                    "frame": { "type": "integer", "description": "Target frame (default: current frame)" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "fill": { "type": "integer", "description": "Fill colour 0xRRGGBB" },
                    "opacity": { "type": "number", "description": "Stroke opacity 0-1 (default 1)" },
                    "scatter": { "type": "number", "description": "Dab jitter radius in stage units, >= 0" }
                }),
                &["kind", "x1", "y1", "x2", "y2"],
            ),
            handler: tool_draw_shape,
        },
        Tool {
            name: "draw_polygon",
            description: "Append a closed regular polygon as a stroke: centred at (cx, cy) with the given radius, 'sides' vertices (default 5, at least 3) plus the closing point, first vertex straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step.",
            schema: obj_schema(
                json!({
                    "cx": { "type": "number" },
                    "cy": { "type": "number" },
                    "radius": { "type": "number" },
                    "sides": { "type": "integer", "minimum": 3, "description": "Vertex count (default 5)" },
                    "frame": { "type": "integer", "description": "Target frame (default: current frame)" },
                    "rotation": { "type": "number", "description": "Rotation in degrees (default 0 = first vertex straight up)" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "fill": { "type": "integer", "description": "Fill colour 0xRRGGBB" },
                    "opacity": { "type": "number", "description": "Stroke opacity 0-1 (default 1)" },
                    "scatter": { "type": "number", "description": "Dab jitter radius in stage units, >= 0" }
                }),
                &["cx", "cy", "radius"],
            ),
            handler: tool_draw_polygon,
        },
        Tool {
            name: "draw_star",
            description: "Append a star outline as a stroke: centred at (cx, cy), outer radius 'radius', inner radius 26/60 of it (the stamp panel's ratio), 'points' spikes (default 5), first spike straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step.",
            schema: obj_schema(
                json!({
                    "cx": { "type": "number" },
                    "cy": { "type": "number" },
                    "radius": { "type": "number" },
                    "points": { "type": "integer", "minimum": 2, "description": "Spike count (default 5)" },
                    "frame": { "type": "integer", "description": "Target frame (default: current frame)" },
                    "rotation": { "type": "number", "description": "Rotation in degrees (default 0 = first spike straight up)" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "fill": { "type": "integer", "description": "Fill colour 0xRRGGBB" },
                    "opacity": { "type": "number", "description": "Stroke opacity 0-1 (default 1)" },
                    "scatter": { "type": "number", "description": "Dab jitter radius in stage units, >= 0" }
                }),
                &["cx", "cy", "radius"],
            ),
            handler: tool_draw_star,
        },
        Tool {
            name: "smooth_strokes",
            description: "Chaikin-smooth every stroke on a frame (one subdivision pass). Returns how many strokes changed.",
            schema: obj_schema(
                json!({ "frame": { "type": "integer" } }),
                &["frame"],
            ),
            handler: tool_smooth_strokes,
        },
        Tool {
            name: "erase_strokes",
            description: "Erase stroke points within radius of (x, y) on a frame, splitting strokes at the cut (draw_ops::erase semantics). Returns how many of the frame's strokes changed.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "radius": { "type": "number" }
                }),
                &["frame", "x", "y", "radius"],
            ),
            handler: tool_erase_strokes,
        },
        Tool {
            name: "simplify_strokes",
            description: "Simplify every stroke on a frame with Douglas-Peucker: interior points within 'epsilon' stage units of the path are dropped, endpoints kept. 'frame' defaults to the current frame. Returns how many strokes lost points. One undo step.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer", "description": "Target frame (default: current frame)" },
                    "epsilon": { "type": "number", "description": "Tolerance in stage units" }
                }),
                &["epsilon"],
            ),
            handler: tool_simplify_strokes,
        },
        Tool {
            name: "delete_stroke",
            description: "Delete one stroke by its index in the project's stroke list (the order draw_stroke appends in). Returns the frame the stroke was on. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_delete_stroke,
        },
        Tool {
            name: "apply_nib",
            description: "Give every stroke on a frame a nib width profile: \"round\" (uniform), \"square\" or \"calligraphy\" (angle-dependent, angle in degrees, default 45). Returns the stroke count.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "nib": { "type": "string", "enum": ["round", "square", "calligraphy"] },
                    "angle": { "type": "number", "description": "Nib angle in degrees (default 45)" }
                }),
                &["frame", "nib"],
            ),
            handler: tool_apply_nib,
        },
        Tool {
            name: "fig3_enter",
            description: "Enter 3D mode: start a session posing the 3D humanoid with an orbit camera. Keeping an existing session as-is. Pose with fig3_select/fig3_rotate/fig3_orbit, then fig3_bake turns the pose into a normal 2D figure.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_fig3_enter,
        },
        Tool {
            name: "fig3_select",
            description: "Select the 3D joint (node index) that fig3_rotate acts on.",
            schema: obj_schema(
                json!({ "joint": { "type": "integer" } }),
                &["joint"],
            ),
            handler: tool_fig3_select,
        },
        Tool {
            name: "fig3_rotate",
            description: "Rotate the selected 3D joint by degrees around an Euler axis: 0 = X, 1 = Y, 2 = Z.",
            schema: obj_schema(
                json!({
                    "axis": { "type": "integer", "enum": [0, 1, 2] },
                    "degrees": { "type": "number" }
                }),
                &["axis", "degrees"],
            ),
            handler: tool_fig3_rotate,
        },
        Tool {
            name: "fig3_orbit",
            description: "Orbit the 3D camera by yaw/pitch deltas in degrees (pitch clamps to +-89.9).",
            schema: obj_schema(
                json!({
                    "yaw": { "type": "number" },
                    "pitch": { "type": "number" }
                }),
                &["yaw", "pitch"],
            ),
            handler: tool_fig3_orbit,
        },
        Tool {
            name: "fig3_bake",
            description: "Bake the current 3D pose into a new 2D figure in the project (held on all frames, like the editor's Bake 3D). Returns the new figure index.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_fig3_bake,
        },
        Tool {
            name: "fig3_mark_b",
            description: "Mark the current 3D pose as pose B, the target fig3_bake_animation interpolates to (pose the figure, mark B, repose to A, then bake the animation).",
            schema: obj_schema(json!({}), &[]),
            handler: tool_fig3_mark_b,
        },
        Tool {
            name: "fig3_bake_animation",
            description: "Bake an animated 2D figure: interpolate the 3D session's current pose to the marked pose B over 'frames' frames (at least 2), extending the timeline if needed. Requires fig3_mark_b first.",
            schema: obj_schema(
                json!({ "frames": { "type": "integer", "minimum": 2 } }),
                &["frames"],
            ),
            handler: tool_fig3_bake_animation,
        },
        Tool {
            name: "clip_create",
            description: "Create a movieclip from a figure's poses across all frames and set the figure to loop it (the editor's Make clip). Returns the clip index and frame count.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &["figure"],
            ),
            handler: tool_clip_create,
        },
        Tool {
            name: "clip_list",
            description: "List the project's movieclip definitions: index, name, frame count, node count, per-node segment media, and which figures loop each clip.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_list,
        },
        Tool {
            name: "set_clip_loop",
            description: "Set a movieclip's loop mode: with 'mode' (\"loop\", \"ping-pong\" or \"once\") the mode is set directly (one undo step; setting the current mode is a no-op); without it, the mode cycles one step: Loop -> PingPong -> Once -> Loop. The reply names the resulting mode; an out-of-range index is a no-op reported in the status.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "mode": { "type": "string", "enum": ["loop", "ping-pong", "once"], "description": "Set this mode directly instead of cycling" }
                }),
                &["index"],
            ),
            handler: tool_set_clip_loop,
        },
        Tool {
            name: "figure_list_states",
            description: "List a figure's animation states: its base clip link and its state keys (frame, clip index + name, offset, speed) in timeline order. Read-only.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &["figure"],
            ),
            handler: tool_figure_list_states,
        },
        Tool {
            name: "figure_set_state",
            description: "Author (or replace) a figure's animation-state key at 'frame': from that frame the figure plays the given clip, starting 'offset' frames in (default 0) at 'speed' times the timeline rate (default 1.0). The clip is given by index ('clip') or by name ('clip_name'). The clip must fit the figure's rig (see clip_compatible); a key already at the frame is replaced wholesale, so an omitted offset/speed resets to the defaults. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" },
                    "clip": { "type": "integer" },
                    "clip_name": { "type": "string", "description": "Clip name instead of the 'clip' index" },
                    "offset": { "type": "integer", "description": "Frames into the clip where playback starts (default 0)" },
                    "speed": { "type": "number", "description": "Playback rate multiplier (default 1.0)" }
                }),
                &["figure", "frame"],
            ),
            handler: tool_figure_set_state,
        },
        Tool {
            name: "figure_clear_state",
            description: "Remove a figure's animation-state key at 'frame'. Refuses when the figure has no key there. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" }
                }),
                &["figure", "frame"],
            ),
            handler: tool_figure_clear_state,
        },
        Tool {
            name: "figure_assign_clip",
            description: "Link an existing movieclip to a figure as its base clip (the clip that plays before the first state key, and whenever the figure has no keys), given by index ('clip') or by name ('clip_name'). Refused when the clip's rig does not fit the figure (see clip_compatible). Assigning the clip already linked is a no-op. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "clip": { "type": "integer" },
                    "clip_name": { "type": "string", "description": "Clip name instead of the 'clip' index" }
                }),
                &["figure"],
            ),
            handler: tool_figure_assign_clip,
        },
        Tool {
            name: "figure_unlink_clip",
            description: "Unlink a figure's base movieclip: the figure falls back to its timeline poses. State keys are NOT cleared - they still apply. Unlinking an unlinked figure is a no-op. One undo step.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &["figure"],
            ),
            handler: tool_figure_unlink_clip,
        },
        Tool {
            name: "clip_compatible",
            description: "Check whether a movieclip (by 'clip' index or 'clip_name') can play on a figure: the clip's rig must match the figure's node count and every clip frame must fit that rig. Read-only.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "clip": { "type": "integer" },
                    "clip_name": { "type": "string", "description": "Clip name instead of the 'clip' index" }
                }),
                &["figure"],
            ),
            handler: tool_clip_compatible,
        },
        Tool {
            name: "clip_create_range",
            description: "Create a movieclip from a figure's poses over the half-open frame range [start, end) and link the figure to it (the ranged sibling of clip_create). Returns the clip index and frame count. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "start": { "type": "integer" },
                    "end": { "type": "integer" }
                }),
                &["figure", "start", "end"],
            ),
            handler: tool_clip_create_range,
        },
        Tool {
            name: "set_bend",
            description: "Set one segment's bend (radians, clamped to +/-0.7) for a figure on a frame. Positive/negative bend arcs the segment to opposite sides; 0 is straight. Frame.bends rows/cols are extended with zeros as needed. Node 0 (the origin) has no segment to bend and is refused. The reply reports the stored bend in radians and degrees. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" },
                    "node": { "type": "integer" },
                    "radians": { "type": "number" }
                }),
                &["figure", "frame", "node", "radians"],
            ),
            handler: tool_set_bend,
        },
        Tool {
            name: "figure_recolor",
            description: "Recolour a whole figure: write the colour (0xRRGGBB) as every segment's own colour override, under one undo step. Gradient end colours and the figure's base colour are left alone.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["figure", "color"],
            ),
            handler: crate::figure_ops::tool_figure_recolor,
        },
        Tool {
            name: "figure_rename",
            description: "Rename a figure (the name is trimmed; blank is refused). A plain field write like the inspector's Rename — not an undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &["figure", "name"],
            ),
            handler: crate::figure_ops::tool_figure_rename,
        },
        Tool {
            name: "figure_scale",
            description: "Scale a figure's segment lengths by 'factor' (1.1 grows 10%, 0.9 shrinks): every parented segment's length is multiplied and clamped to [4, 220], as one undo step. Refuses when the factor is not a positive number, or when every segment already sits at the clamp in that direction. The reply's status reports the achieved scale. 'figure' defaults to the selected figure (which becomes the selection, as with delete_figure/move_figure).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "factor": { "type": "number" }
                }),
                &["factor"],
            ),
            handler: crate::figure_ops::tool_figure_scale,
        },
        Tool {
            name: "figure_centre",
            description: "Centre a figure on the stage at the current frame: shift its root so its drawn bounding box (head ring included) centres on the stage. Current frame only. One undo step. 'figure' defaults to the selected figure.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_figure_centre,
        },
        Tool {
            name: "figure_ground",
            description: "Ground a figure at the current frame: shift its root down so its drawn bounding box's bottom rests exactly on the stage's bottom edge. Current frame only. One undo step. 'figure' defaults to the selected figure.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_figure_ground,
        },
        Tool {
            name: "copy_pose_between",
            description: "Copy the current frame's pose of one figure onto another figure (same frame), as one undo step — the pose panel's Copy-pose-to. Refuses when either figure has no pose on the current frame, when source and target are the same figure, or when the two poses have different angle counts (different rig sizes).",
            schema: obj_schema(
                json!({
                    "from_figure": { "type": "integer" },
                    "to_figure": { "type": "integer" }
                }),
                &["from_figure", "to_figure"],
            ),
            handler: crate::figure_ops::tool_copy_pose_between,
        },
        Tool {
            name: "reset_pose",
            description: "Reset a figure's pose in a frame to its rest pose (root position kept), as one undo step — the pose panel's Reset pose. 'figure' defaults to the selected figure, 'frame' to the current frame.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_reset_pose,
        },
        Tool {
            name: "set_rest_pose",
            description: "Set a figure's rest (setup) pose from the pose it shows at the current frame (the timeline pose, clips and state keys applied), as one undo step — the figure inspector's Set-rest-from-current-pose. With 'rebase' (the default), every authored angle — frame poses, the frames of clips authored on the figure's rig, and its Angle graph channels — shifts by the per-node rest delta, so the animation keeps its content against the new rig; with 'rebase' false only the rest angles change and the timeline is untouched (absolute). Setting the rest to the pose that already is the rest pose is a no-op and spends no undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "rebase": { "type": "boolean", "description": "Shift the authored animation by the rest delta (default true); false sets the rest only" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_set_rest_pose,
        },
        Tool {
            name: "pose_apply_all_frames",
            description: "Write a pose onto a figure in EVERY frame, as one undo step — the pose panel's Apply-pose-to-all-frames with the pose supplied in the call. Frames whose pose row is missing or whose angle count differs from the supplied angles are skipped and counted in the reply. 'root' is optional: omit it to keep each frame's own root position.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "angles": { "type": "array", "items": { "type": "number" }, "description": "One angle per node, degrees" },
                    "root": { "type": "array", "items": { "type": "number" }, "description": "[x, y]; omit to keep each frame's root" }
                }),
                &["figure", "angles"],
            ),
            handler: crate::figure_ops::tool_pose_apply_all_frames,
        },
        Tool {
            name: "pose_library_list",
            description: "List the poses saved in the 2D pose library (./library/poses) by display name, sorted — the pose panel's library list.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::figure_ops::tool_pose_library_list,
        },
        Tool {
            name: "pose_library_save",
            description: "Save a figure's pose from a frame into the pose library under 'name' (default: the figure's own name — saving the same figure again overwrites its entry, as the pose panel's Save does). A file write, not an undo step. 'figure' defaults to the selected figure, 'frame' to the current frame.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_pose_library_save,
        },
        Tool {
            name: "pose_library_apply",
            description: "Apply a saved pose-library pose to a figure in a frame, as one undo step — the pose panel's library Apply. With 'mirror': true the pose is mirrored (angles negated) as it is applied. Refuses when the entry cannot be loaded, the figure has no pose in the frame, or the saved pose's angle count does not match the figure's rig. 'figure' defaults to the selected figure, 'frame' to the current frame.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" },
                    "figure": { "type": "integer" },
                    "frame": { "type": "integer" },
                    "mirror": { "type": "boolean" }
                }),
                &["name"],
            ),
            handler: crate::figure_ops::tool_pose_library_apply,
        },
        Tool {
            name: "pose_library_delete",
            description: "Delete a pose from the pose library (removes its file in ./library/poses). A file operation, not an undo step.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" }
                }),
                &["name"],
            ),
            handler: crate::figure_ops::tool_pose_library_delete,
        },
        Tool {
            name: "pose_library_rename",
            description: "Rename a pose-library entry. Refuses a blank new name and any name another entry already owns (including a different display name that slugs to the same library file). Renaming to the entry's own name succeeds with renamed=false. A file operation, not an undo step.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" },
                    "new_name": { "type": "string" }
                }),
                &["name", "new_name"],
            ),
            handler: crate::figure_ops::tool_pose_library_rename,
        },
        Tool {
            name: "figure_library_rename",
            description: "Rename a saved figure-library entry (the library panel's per-row Rename): renames the entry's backing file in ./library and updates the entry in place, addressed by its library index. The new name is sanitised to a file stem (trimmed, spaces and path separators become '-', lowercased). Refuses built-in entries, a name that sanitises to empty, a name another entry already owns, and a target file that already exists in the library folder. Renaming to the entry's own current name succeeds with renamed=false. A file operation, not an undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "new_name": { "type": "string" }
                }),
                &["index", "new_name"],
            ),
            handler: crate::library_ops::tool_figure_library_rename,
        },
        Tool {
            name: "layers_set_all_visible",
            description: "Show or hide ALL figures at once (the figures panel's All / None buttons): visible=true shows every figure, visible=false hides every figure. Clears any active solo, exactly as the panel does, so the flags are the effective visibility. Returns how many figures changed state. Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(
                json!({
                    "visible": { "type": "boolean" }
                }),
                &["visible"],
            ),
            handler: crate::figure_ops::tool_layers_set_all_visible,
        },
        Tool {
            name: "layers_invert_visibility",
            description: "Invert every figure's hidden flag (the figures panel's Invert button). Clears any active solo, exactly as the panel does. Returns how many figures changed state (always the figure count). Not an undo step (the panel writes through the same mark_dirty seam); a real change counts for dirty/autosave.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::figure_ops::tool_layers_invert_visibility,
        },
        Tool {
            name: "list_strokes",
            description: "List strokes in the project (the stroke panel's list, as data): index in the project-wide stroke list, frame, point count, color (0xRRGGBB), width, per-point widths count, fill, spray mark, opacity and scatter. 'frame' filters to one frame; 'points': true includes each stroke's full polyline instead of its point count.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer", "description": "Only list strokes on this frame" },
                    "points": { "type": "boolean", "description": "Include full point lists (default false)" }
                }),
                &[],
            ),
            handler: crate::draw_ops::tool_list_strokes,
        },
        Tool {
            name: "get_drawing_stats",
            description: "Drawing totals over the whole project (the Draw panel's readout, as data): stroke count, total points, summed polyline length in stage units, spray stroke count, and the current frame's stroke count.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_get_drawing_stats,
        },
        Tool {
            name: "draw_arrow",
            description: "Append an arrow as a stroke: shaft from (x1, y1) to the tip (x2, y2) with a two-wing head of length 'head' (default 22, the editor's stamp). Geometry is paint::arrow_points, the Draw panel's Arrow stamp generator. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width. One undo step.",
            schema: obj_schema(
                json!({
                    "x1": { "type": "number" },
                    "y1": { "type": "number" },
                    "x2": { "type": "number" },
                    "y2": { "type": "number" },
                    "head": { "type": "number", "description": "Head wing length in stage units (default 22)" },
                    "frame": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" }
                }),
                &["x1", "y1", "x2", "y2"],
            ),
            handler: crate::draw_ops::tool_draw_arrow,
        },
        Tool {
            name: "set_stroke_style",
            description: "Edit one stroke's style by its project-wide index (see list_strokes): color (0xRRGGBB), width (0.5-40, the stroke panel's range; a per-point taper scales proportionally), opacity (0-1), scatter (>= 0), fill (0xRRGGBB) or clear_fill. At least one field is required. Opacity and scatter are refused on spray strokes — a spray stroke's opacity lives on its spray mark and it never renders through the scatter path. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "opacity": { "type": "number" },
                    "scatter": { "type": "number", "description": "Dab jitter radius in stage units, >= 0" },
                    "fill": { "type": "integer", "description": "Fill colour 0xRRGGBB" },
                    "clear_fill": { "type": "boolean", "description": "Remove the stroke's fill" }
                }),
                &["index"],
            ),
            handler: crate::draw_ops::tool_set_stroke_style,
        },
        Tool {
            name: "simplify_stroke",
            description: "Simplify ONE stroke by its project-wide index with Douglas-Peucker (the stroke panel's Simplify press): interior points within 'epsilon' stage units (default 2) are dropped, that point's own width survives. Returns simplified: false — and takes no undo step — when the stroke is already simple at that epsilon. One undo step when it simplifies.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "epsilon": { "type": "number", "description": "Tolerance in stage units (default 2)" }
                }),
                &["index"],
            ),
            handler: crate::draw_ops::tool_simplify_stroke,
        },
        Tool {
            name: "fill_stroke",
            description: "Fill the most recent stroke on the CURRENT frame: its fill is set to its own colour and a scanline fill is appended as a companion stroke (EditorCore::fill_current_stroke). Errors when there is nothing fillable on the frame. One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_fill_stroke,
        },
        Tool {
            name: "lasso_erase",
            description: "Erase strokes on the CURRENT frame that lie mostly (a strict majority of their points) inside the lasso polygon 'points' (at least 3 [x, y] points). Errors when nothing lies mostly inside. One undo step when strokes are removed.",
            schema: obj_schema(
                json!({
                    "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } }
                }),
                &["points"],
            ),
            handler: crate::draw_ops::tool_lasso_erase,
        },
        Tool {
            name: "mirror_strokes",
            description: "Mirror every stroke on the CURRENT frame across the stage's vertical centre line (the Draw panel's Mirror strokes button). Only points change. Errors on a frame with no strokes. One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_mirror_strokes,
        },
        Tool {
            name: "clear_frame_strokes",
            description: "Remove every stroke on the current frame (the stroke list's Clear button). 'frame' may only name the current frame — the op is current-frame-only and errors otherwise rather than clearing the wrong frame. One undo step when the frame carried strokes.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer", "description": "Must be the current frame when given" }
                }),
                &[],
            ),
            handler: crate::draw_ops::tool_clear_frame_strokes,
        },
        Tool {
            name: "delete_all_strokes",
            description: "Delete every stroke in the project, on all frames (the stroke panel's Delete all). One undo step for the whole deletion.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_delete_all_strokes,
        },
        Tool {
            name: "recolor_strokes",
            description: "Recolour strokes to 'color' (0xRRGGBB, required): scope 'frame' recolours the current frame's strokes (the panel's Frame Recolour), scope 'all' recolours every stroke in the project (the panel's Recolour all — except the panel's target colour is the selected figure's, while here it is explicit). One undo step when strokes match.",
            schema: obj_schema(
                json!({
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "scope": { "type": "string", "enum": ["frame", "all"], "description": "Default 'frame'" }
                }),
                &["color"],
            ),
            handler: crate::draw_ops::tool_recolor_strokes,
        },
        Tool {
            name: "list_brushes",
            description: "List the brush preset library (the Draw panel's brush chips): index, name, width, nib, nib angle, smoothing, opacity and scatter per preset, plus the active preset's index and name.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_list_brushes,
        },
        Tool {
            name: "set_brush",
            description: "Select the active brush preset by 'name' (exact, then case-insensitive) or 'index' — the Draw panel's brush chips. Selection only; draws nothing. Subsequent interactive drawing, stamps and the eraser size use the selected preset. On the live transport this changes the editor's active brush.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string", "description": "Preset name, e.g. \"Spray\"" },
                    "index": { "type": "integer" }
                }),
                &[],
            ),
            handler: crate::draw_ops::tool_set_brush,
        },
        Tool {
            name: "cycle_brush",
            description: "Cycle to the next brush preset in the library, wrapping (the Draw panel's brush-cycle button). Returns the new active preset.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::draw_ops::tool_cycle_brush,
        },
        Tool {
            name: "set_draw_tool",
            description: "Set the active draw tool: 'brush' or 'eraser' (the Draw panel's tool toggle, as an idempotent set). Affects subsequent interactive drawing and eraser sizing; draws nothing itself.",
            schema: obj_schema(
                json!({
                    "tool": { "type": "string", "enum": ["brush", "eraser"] }
                }),
                &["tool"],
            ),
            handler: crate::draw_ops::tool_set_draw_tool,
        },
        Tool {
            name: "get_node_style",
            description: "Read a segment's complete style: colour override (null = inherits the figure colour), effective colour, gradient end colour, cap, taper and opacity.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" }
                }),
                &["figure", "node"],
            ),
            handler: crate::style::tool_get_node_style,
        },
        Tool {
            name: "cycle_node_taper",
            description: "Step a segment's taper one step (1.0 -> 0.6 -> 0.3 -> 0.0 -> 1.0), exactly the style panel's taper Step button. A taper set directly snaps to the next step down. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" }
                }),
                &["figure", "node"],
            ),
            handler: crate::style::tool_cycle_node_taper,
        },
        Tool {
            name: "set_node_taper",
            description: "Set a segment's taper directly (0.0-1.0; 1.0 = uniform width, lower = sharper point). Values outside 0.0-1.0 are refused, not clamped. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "taper": { "type": "number", "description": "Far-end width multiplier, 0.0-1.0" }
                }),
                &["figure", "node", "taper"],
            ),
            handler: crate::style::tool_set_node_taper,
        },
        Tool {
            name: "set_node_style",
            description: "Write a segment's complete style (colour, gradient end, cap, opacity) in exactly one undo step — the write half of the style panel's copy/paste; pair with get_node_style to copy a style between segments. Omit 'color2' (or pass null) to clear the gradient. Taper is not part of this write; use set_node_taper. Opacity below 0 is refused; any other value is clamped to 0.05-1.0 and read back as stored.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "color2": { "type": "integer", "description": "0xRRGGBB gradient end colour" },
                    "cap": { "type": "string", "enum": ["round", "square", "point"] },
                    "opacity": { "type": "number", "description": "Opacity; below 0 refused, otherwise clamped to 0.05-1.0" }
                }),
                &["figure", "node", "color", "cap", "opacity"],
            ),
            handler: crate::style::tool_set_node_style,
        },
        Tool {
            name: "apply_style_preset",
            description: "Apply one of the style panel's six curated presets (Ink, Crimson, Ocean, Forest, Violet, Amber) to a segment: the preset's colour and cap are set, the segment's gradient end and opacity are left untouched. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "preset": { "type": "string", "enum": ["Ink", "Crimson", "Ocean", "Forest", "Violet", "Amber"] }
                }),
                &["figure", "node", "preset"],
            ),
            handler: crate::style::tool_apply_style_preset,
        },
        Tool {
            name: "list_texts",
            description: "List every text in the project: segment texts (figure, node, content, size) and free-standing text instances in draw order (index, content, size, colour, transform keys).",
            schema: obj_schema(json!({}), &[]),
            handler: crate::text_edit::tool_list_texts,
        },
        Tool {
            name: "set_segment_text",
            description: "Set (or replace) the text on a figure's segment — the text panel's Add text. Optional 'size' (glyph height) is clamped to 4-160; without it an existing size is preserved and a fresh text gets 28. Empty or control-only content is refused. The response reads back what actually stored (the text session drops control characters such as newlines). One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "content": { "type": "string" },
                    "size": { "type": "number", "description": "Glyph height in stage units, clamped to 4-160" }
                }),
                &["figure", "node", "content"],
            ),
            handler: crate::text_edit::tool_set_segment_text,
        },
        Tool {
            name: "set_segment_text_size",
            description: "Set the glyph height of the text already on a segment (clamped to 4-160). A segment carrying no text is a no-op (changed: false). One undo step when the size changes.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "size": { "type": "number", "description": "Glyph height in stage units, clamped to 4-160" }
                }),
                &["figure", "node", "size"],
            ),
            handler: crate::text_edit::tool_set_segment_text_size,
        },
        Tool {
            name: "clear_segment_text",
            description: "Remove the text from a segment. A segment carrying an image (or no media) is left untouched (changed: false). One undo step when a text is cleared.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" }
                }),
                &["figure", "node"],
            ),
            handler: crate::text_edit::tool_clear_segment_text,
        },
        Tool {
            name: "add_text_instance",
            description: "Create a free-standing text instance (stage label) with one transform key. Defaults: size 28, colour white, key on the current frame at the stage centre. Note: the editor's text panel cannot create instances (they arrive via import) — this tool completes that side of the surface for MCP. One undo step.",
            schema: obj_schema(
                json!({
                    "content": { "type": "string" },
                    "size": { "type": "number", "description": "Glyph height in stage units, clamped to 4-160" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "frame": { "type": "number", "description": "Frame of the instance's transform key" },
                    "x": { "type": "number", "description": "Stage x of the transform key" },
                    "y": { "type": "number", "description": "Stage y of the transform key" }
                }),
                &["content"],
            ),
            handler: crate::text_edit::tool_add_text_instance,
        },
        Tool {
            name: "update_text_instance",
            description: "Rewrite a text instance's content (the text panel's Update text): position keys, size and colour are untouched. Empty content is refused — an update never blanks a text; unchanged content is a no-op (changed: false). One undo step when the content changes.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "content": { "type": "string" }
                }),
                &["index", "content"],
            ),
            handler: crate::text_edit::tool_update_text_instance,
        },
        Tool {
            name: "delete_text_instance",
            description: "Delete a free-standing text instance. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::text_edit::tool_delete_text_instance,
        },
        Tool {
            name: "duplicate_text_instance",
            description: "Duplicate a text instance: the copy is inserted directly after the original, shifted +12/+12 stage units (the text panel's Duplicate). Returns the copy's index. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::text_edit::tool_duplicate_text_instance,
        },
        Tool {
            name: "nudge_text_instance",
            description: "Nudge a text instance by (dx, dy) stage units: every transform key shifts by the same delta. An instance with no keys has no position to move (changed: false). One undo step when it moves.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "dx": { "type": "number" },
                    "dy": { "type": "number" }
                }),
                &["index", "dx", "dy"],
            ),
            handler: crate::text_edit::tool_nudge_text_instance,
        },
        Tool {
            name: "set_text_instance_size",
            description: "Set a text instance's glyph height (clamped to 4-160, the text panel's size range). Ok with changed: false when the stored size already equals the clamped request. One undo step when the size changes.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "size": { "type": "number", "description": "Glyph height in stage units, clamped to 4-160" }
                }),
                &["index", "size"],
            ),
            handler: crate::text_edit::tool_set_text_instance_size,
        },
        Tool {
            name: "rotate_text_instance",
            description: "Rotate a text instance by the given degrees: every transform key's angle gains the same amount (the text panel rotates in 15-degree steps). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "degrees": { "type": "number" }
                }),
                &["index", "degrees"],
            ),
            handler: crate::text_edit::tool_rotate_text_instance,
        },
        Tool {
            name: "set_text_instance_color",
            description: "Set a text instance's colour (0xRRGGBB). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "color": { "type": "integer", "description": "0xRRGGBB" }
                }),
                &["index", "color"],
            ),
            handler: crate::text_edit::tool_set_text_instance_color,
        },
        Tool {
            name: "move_text_instance",
            description: "Send a text instance to an end of the draw order: to_front true draws it on top of the other texts, false puts it behind them. Already at that end is a no-op (changed: false). One undo step when it moves.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "to_front": { "type": "boolean" }
                }),
                &["index", "to_front"],
            ),
            handler: crate::text_edit::tool_move_text_instance,
        },
        Tool {
            name: "swap_frame_with_neighbour",
            description: "Swap the current frame with its 'previous' or 'next' neighbour (the timeline's frame-swap buttons): poses, durations, easings and bends trade places. One undo step; refuses at either end of the timeline.",
            schema: obj_schema(
                json!({
                    "direction": { "type": "string", "enum": ["previous", "next"] }
                }),
                &["direction"],
            ),
            handler: crate::frame_swap::tool_swap_frame_with_neighbour,
        },
        Tool {
            name: "insert_frames",
            description: "Insert 'count' blank frames immediately after the current frame (the timeline's batch insert; count is clamped to 60 by the op). The inserted frames copy the current frame's timing. One undo step.",
            schema: obj_schema(
                json!({
                    "count": { "type": "integer", "description": "Frames to insert, at least 1" }
                }),
                &["count"],
            ),
            handler: crate::frame_swap::tool_insert_frames,
        },
        Tool {
            name: "get_keyframes",
            description: "Read one figure's keyframe list — the exact data the dopesheet lane draws (a frame is a keyframe when the figure's pose there differs from the previous frame's). Read-only.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" }
                }),
                &["figure"],
            ),
            handler: crate::frame_swap::tool_get_keyframes,
        },
        Tool {
            name: "list_frames",
            description: "Read the timeline's per-frame timing table: every frame's duration multiplier and easing override, plus the playhead and loop range — the read counterpart of set_frame_duration / set_frame_easing. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::frame_swap::tool_list_frames,
        },
        Tool {
            name: "fx_set_hidden",
            description: "Hide or show one FX clip (the FX panel's hide toggle): a hidden clip renders nowhere — stage, raster and SVG all skip it. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "hidden": { "type": "boolean" }
                }),
                &["index", "hidden"],
            ),
            handler: crate::fx_ops::tool_fx_set_hidden,
        },
        Tool {
            name: "fx_preset_list",
            description: "List the FX clip presets saved in ./fx-presets/ by name, sorted — the read side of fx_preset_save / fx_preset_add (the panel's preset list). Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fx_ops::tool_fx_preset_list,
        },
        Tool {
            name: "particle_list_events",
            description: "List every particle event in the project: index, frame, stage position, kind, colour and anchor (figure/node) — the read the Particles panel rows show. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::particle_ops::tool_particle_list_events,
        },
        Tool {
            name: "particle_delete_event",
            description: "Delete the particle event at 'index' (the Particles panel's Delete). Events after it shift down one index — re-read with particle_list_events. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::particle_ops::tool_particle_delete_event,
        },
        Tool {
            name: "particle_set_frame",
            description: "Move the particle event at 'index' to 0-based 'frame', clamped to the project's last frame (the panel's frame stepper and To playhead). The reply reports the frame the event actually landed on. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "frame": { "type": "integer" }
                }),
                &["index", "frame"],
            ),
            handler: crate::particle_ops::tool_particle_set_frame,
        },
        Tool {
            name: "particle_nudge_frame",
            description: "Move the particle event at 'index' by signed 'delta' frames, clamped at both ends of the timeline. The reply reports the frame the event actually landed on. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "delta": { "type": "integer" }
                }),
                &["index", "delta"],
            ),
            handler: crate::particle_ops::tool_particle_nudge_frame,
        },
        Tool {
            name: "particle_set_position",
            description: "Move the particle event at 'index' to the stage point (x, y) — the point its effect renders from, independent of its anchor (the panel's nudge pad writes this in 4-unit steps). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "x": { "type": "number" },
                    "y": { "type": "number" }
                }),
                &["index", "x", "y"],
            ),
            handler: crate::particle_ops::tool_particle_set_position,
        },
        Tool {
            name: "particle_set_anchor",
            description: "Anchor the particle event at 'index' to figure 'figure', joint 'node' (the panel's Anchor to figure / Anchor to joint), or clear the anchor back to a free stage event by passing 'figure' as null or omitting it (the panel's Clear anchor). The figure/node are validated before anything changes. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "figure": { "type": "integer", "description": "Figure index, or null to clear the anchor" },
                    "node": { "type": "integer", "description": "Joint index on the figure" }
                }),
                &["index"],
            ),
            handler: crate::particle_ops::tool_particle_set_anchor,
        },
        Tool {
            name: "particle_add_anchored",
            description: "Add a particle event anchored to the SELECTED figure's selected joint at the current frame (the panel's anchored add). 'kind' is a particle kind name (default burst). Refuses when the selection cannot anchor an event. One undo step.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "description": "Particle kind name, e.g. \"burst\"" }
                }),
                &[],
            ),
            handler: crate::particle_ops::tool_particle_add_anchored,
        },
        Tool {
            name: "draw_spray",
            description: "Append a spray-mark stroke on a frame (the Spray brush's stroke kind — draw_stroke cannot create one): dots are scattered around 'points' within 'radius'. 'frame' and 'points' are required; color (0xRRGGBB), width, radius, opacity and seed default from the active brush preset. One undo step.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" },
                    "radius": { "type": "number", "description": "Spray radius in stage units" },
                    "opacity": { "type": "number" },
                    "seed": { "type": "integer" }
                }),
                &["frame", "points"],
            ),
            handler: crate::paint::tool_draw_spray,
        },
        Tool {
            name: "get_stroke_style",
            description: "Read one stroke's style back by its project-wide index (see list_strokes): colour, width, per-point widths, fill, spray mark, opacity and scatter — the read half of set_stroke_style. Read-only.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::paint::tool_get_stroke_style,
        },
        Tool {
            name: "audio_list_clips",
            description: "List every audio clip with its stored fields (index, path, start frame, gain, muted) and probed health (healthy / missing / unsupported / unreadable, with duration) — the read the audio panel's rows show. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::audio_ops::tool_audio_list_clips,
        },
        Tool {
            name: "audio_clip_info",
            description: "Read one audio clip's full facts by 'index': stored fields, probed health, duration in seconds and frames, peak level, and waveform peaks at 'buckets' buckets (default 64; an empty peaks array means the file could not be decoded — see health). Read-only.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "buckets": { "type": "integer", "description": "Waveform peak buckets (default 64)" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_info,
        },
        Tool {
            name: "audio_clip_fade",
            description: "Fade the audio clip at 'index' (0.25 s in/out, the panel's per-clip Fade) — the indexed form the panel has; the older audio_fade tool always edits the LAST clip. A clip whose file cannot be read is an error, not ok. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_fade,
        },
        Tool {
            name: "audio_clip_reverse",
            description: "Reverse the audio clip at 'index' — the indexed form of audio_reverse (which always edits the last clip). The processed file is written beside the source and the clip repointed. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_reverse,
        },
        Tool {
            name: "audio_clip_set_gain",
            description: "Set the audio clip at 'index' gain (clamped to 0.0-2.0 by the op; the reply reports the gain read back after clamping, so a request for 5.0 truthfully reports 2.0) — the indexed form of audio_set_gain. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "gain": { "type": "number" }
                }),
                &["index", "gain"],
            ),
            handler: crate::audio_ops::tool_audio_clip_set_gain,
        },
        Tool {
            name: "audio_clip_move",
            description: "Move the audio clip at 'index' so it starts at the absolute 0-based 'start_frame' (the reply reports the actual start). One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "start_frame": { "type": "integer" }
                }),
                &["index", "start_frame"],
            ),
            handler: crate::audio_ops::tool_audio_clip_move,
        },
        Tool {
            name: "audio_clip_duplicate",
            description: "Duplicate the audio clip at 'index' (the panel's Duplicate button): the copy is inserted immediately after the original at the same start frame; the reply names its index. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_duplicate,
        },
        Tool {
            name: "audio_clip_normalise",
            description: "Normalise the audio clip at 'index' FILE to peak 0.9 (the panel's per-row Norm button): the processed copy is written beside the source and the clip repointed; the clip's gain field is untouched. Distinct from audio_normalise_all, which is gain-based. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_normalise,
        },
        Tool {
            name: "audio_clip_trim",
            description: "Trim the audio clip at 'index' file to the range [start_secs, end_secs) — SECONDS, not frames: the clip's start frame advances by round(start_secs * fps), preserving the surviving audio's timeline position; the reply reports the new start frame. A range entirely beyond the file is refused. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "start_secs": { "type": "number" },
                    "end_secs": { "type": "number" }
                }),
                &["index", "start_secs", "end_secs"],
            ),
            handler: crate::audio_ops::tool_audio_clip_trim,
        },
        Tool {
            name: "audio_clip_toggle_muted",
            description: "Toggle the audio clip at 'index' mute flag (the panel toolbar's Mute toggle); the reply reports the flag's new value, read back from the clip. audio_set_muted is the absolute form. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: crate::audio_ops::tool_audio_clip_toggle_muted,
        },
        Tool {
            name: "fig3_exit",
            description: "Exit 3D mode (the panel's mode toggle / the palette's Exit 3D mode). Baked 2D figures stay in the project.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_exit,
        },
        Tool {
            name: "fig3_get",
            description: "Read the full 3D session state: figure, pose, selection, camera (yaw both raw and wrapped to -180..180 as 'yaw_wrapped' — the panel displays the wrapped value) and library state. Without a session, returns ok with active: false. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_get,
        },
        Tool {
            name: "fig3_select_next",
            description: "Cycle the selected 3D joint to the next node (the panel's Joint> button).",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_select_next,
        },
        Tool {
            name: "fig3_cycle_library",
            description: "Load the next 3D library figure, cycling the library (a core op with no panel button; the panel loads by name instead — fig3_load_from_library). Refuses when the library is empty.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_cycle_library,
        },
        Tool {
            name: "fig3_camera_preset",
            description: "Apply a named 3D camera preset — 'front', 'side', 'three-quarter' or 'top' (the panel's Camera row): replaces the session camera.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string", "enum": ["front", "side", "three-quarter", "top"] }
                }),
                &["name"],
            ),
            handler: crate::fig3d::tool_fig3_camera_preset,
        },
        Tool {
            name: "fig3_frame_camera",
            description: "Frame the 3D session figure on the stage (the panel's Frame camera button): replaces the camera with a front camera targeted on the figure's centroid, fitted to the stage's central 80%.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_frame_camera,
        },
        Tool {
            name: "fig3_frame_preview",
            description: "Frame the 3D session figure in the panel preview (the panel's Frame button): keeps the camera's yaw/pitch/FOV and writes back only the fitted target and distance, at the preview viewport's aspect.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_frame_preview,
        },
        Tool {
            name: "fig3_camera_zoom",
            description: "One preview-zoom step on the 3D camera distance: 'dir' +1 zooms in, -1 zooms out (x/divided by 1.25, clamped to 40..=4000 — the panel's zoom buttons).",
            schema: obj_schema(
                json!({
                    "dir": { "type": "integer", "description": "+1 zooms in, -1 zooms out" }
                }),
                &["dir"],
            ),
            handler: crate::fig3d::tool_fig3_camera_zoom,
        },
        Tool {
            name: "fig3_ik",
            description: "Solve two-bone IK for one limb of the 3D figure toward its natural hang: 'limb' is arm_l, arm_r, leg_l or leg_r (the panel's IK buttons).",
            schema: obj_schema(
                json!({
                    "limb": { "type": "string", "enum": ["arm_l", "arm_r", "leg_l", "leg_r"] }
                }),
                &["limb"],
            ),
            handler: crate::fig3d::tool_fig3_ik,
        },
        Tool {
            name: "fig3_pose_mirror",
            description: "Mirror the 3D session pose across the YZ plane (the panel's Mirror pose button). An exact involution: calling it twice restores the pose.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_pose_mirror,
        },
        Tool {
            name: "fig3_pose_library_list",
            description: "List the 3D pose library (the panel's Pose library section). No session needed. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::fig3d::tool_fig3_pose_library_list,
        },
        Tool {
            name: "fig3_pose_save",
            description: "Save the 3D session pose to the pose library (the panel's Save pose button). 'name' defaults to '<figure> pose'. A file write, not an undo step.",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" }
                }),
                &[],
            ),
            handler: crate::fig3d::tool_fig3_pose_save,
        },
        Tool {
            name: "fig3_pose_apply",
            description: "Apply a saved 3D pose-library entry to the session figure (the panel's pose Apply buttons).",
            schema: obj_schema(
                json!({
                    "name": { "type": "string" }
                }),
                &["name"],
            ),
            handler: crate::fig3d::tool_fig3_pose_apply,
        },
        Tool {
            name: "graph_list_channels",
            description: "List every graph channel in project order: index (the index the other graph tools take), figure, target, key count and value/frame stats — the graph panel's channel list and stats block, as data. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::graph_keys::tool_graph_list_channels,
        },
        Tool {
            name: "graph_get_channel",
            description: "Read one graph channel in full by its project-list 'channel' index: every key (frame, value, tangents, interp). With optional 'frame', the reply also carries the channel's evaluated value there (value_at) — the graph panel's value readout. Read-only.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "frame": { "type": "number", "description": "Also evaluate the channel at this frame" }
                }),
                &["channel"],
            ),
            handler: crate::graph_keys::tool_graph_get_channel,
        },
        Tool {
            name: "graph_delete_key",
            description: "Delete the one key at 'frame' from the graph channel at 'channel' index. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "frame": { "type": "number" }
                }),
                &["channel", "frame"],
            ),
            handler: crate::graph_keys::tool_graph_delete_key,
        },
        Tool {
            name: "graph_move_key",
            description: "Move the key nearest 'from_frame' (within 0.5 frames) in the channel at 'channel' index to 'to_frame' — the graph canvas drag: tangents travel with the key, its value travels unless 'value' overrides it, and a key already sitting at the destination is replaced. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "from_frame": { "type": "number" },
                    "to_frame": { "type": "number" },
                    "value": { "type": "number" }
                }),
                &["channel", "from_frame", "to_frame"],
            ),
            handler: crate::graph_keys::tool_graph_move_key,
        },
        Tool {
            name: "graph_set_tangents",
            description: "Set both tangents of the key at 'frame' in the channel at 'channel' index (in_tan / out_tan are the Hermite slopes the graph canvas drag writes). One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "frame": { "type": "number" },
                    "in_tan": { "type": "number" },
                    "out_tan": { "type": "number" }
                }),
                &["channel", "frame", "in_tan", "out_tan"],
            ),
            handler: crate::graph_keys::tool_graph_set_tangents,
        },
        Tool {
            name: "graph_shift_keys",
            description: "Shift EVERY key of the channel at 'channel' index by 'delta' frames (negative = earlier) — the graph panel's shift buttons at full range. Refused whole (never clamped) when delta is 0 or any key would land before frame 0. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "delta": { "type": "number", "description": "Frames to shift by; negative = earlier" }
                }),
                &["channel", "delta"],
            ),
            handler: crate::graph_keys::tool_graph_shift_keys,
        },
        Tool {
            name: "graph_fill_keys",
            description: "Fill the keyless frames of the channel at 'channel' index inside [from, to] with copies of the channel's key at the CURRENT frame (set the playhead first with set_frame). Returns the inserted count. One undo step when it inserts.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "from": { "type": "integer" },
                    "to": { "type": "integer" }
                }),
                &["channel", "from", "to"],
            ),
            handler: crate::graph_keys::tool_graph_fill_keys,
        },
        Tool {
            name: "graph_delete_keys_in_range",
            description: "Delete the keys of the channel at 'channel' index whose frames fall inside [from, to], both ends inclusive. Returns the removed count. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "from": { "type": "number" },
                    "to": { "type": "number" }
                }),
                &["channel", "from", "to"],
            ),
            handler: crate::graph_keys::tool_graph_delete_keys_in_range,
        },
        Tool {
            name: "graph_clear_channel_keys",
            description: "Remove every key from the channel at 'channel' index (the channel itself stays, empty). One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" }
                }),
                &["channel"],
            ),
            handler: crate::graph_keys::tool_graph_clear_channel_keys,
        },
        Tool {
            name: "graph_reduce_keys",
            description: "Reduce the channel at 'channel' index: drop interior keys whose removal moves the curve by at most 'tolerance' value units (default 0.5, the graph panel's Reduce) — endpoints always kept. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "tolerance": { "type": "number", "description": "Default 0.5" }
                }),
                &["channel"],
            ),
            handler: crate::graph_keys::tool_graph_reduce_keys,
        },
        Tool {
            name: "graph_mirror_channel",
            description: "Mirror the channel at 'channel' index in time around 'center' (or 'centre'; default: the timeline's middle frame) — the index form of mirror_channel_time, which can also name a duplicate channel. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "center": { "type": "number", "description": "Mirror centre frame (default: middle of the timeline)" },
                    "centre": { "type": "number", "description": "Alias of center" }
                }),
                &["channel"],
            ),
            handler: crate::graph_keys::tool_graph_mirror_channel,
        },
        Tool {
            name: "graph_scale_channel",
            description: "Scale every key value of the channel at 'channel' index by 'factor' around 'pivot' (default 0) — the index form of scale_channel_values. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "factor": { "type": "number" },
                    "pivot": { "type": "number", "description": "Default 0" }
                }),
                &["channel", "factor"],
            ),
            handler: crate::graph_keys::tool_graph_scale_channel,
        },
        Tool {
            name: "graph_tangent_preset_channel",
            description: "Apply a tangent preset ('smooth', 'ease-in', 'ease-out', 'ease-in-out', 'linear') to every key of the channel at 'channel' index — the index form of graph_tangent_preset. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "preset": { "type": "string" }
                }),
                &["channel", "preset"],
            ),
            handler: crate::graph_keys::tool_graph_tangent_preset_channel,
        },
        Tool {
            name: "graph_key_all_channels",
            description: "Key every graph channel at 'frame' (default: the current frame) with its evaluated value there — the keyframe tools' Key-all-channels. One undo step.",
            schema: obj_schema(
                json!({
                    "frame": { "type": "number", "description": "Default: the current frame" }
                }),
                &[],
            ),
            handler: crate::graph_keys::tool_graph_key_all_channels,
        },
        Tool {
            name: "graph_key_angle",
            description: "Key the selected node's current pose angle into its graph channel at the current frame (the graph panel's Key angle button — EditorCore::key_graph_angle). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::graph_keys::tool_graph_key_angle,
        },
        Tool {
            name: "graph_clear_channels",
            description: "Remove ALL graph channels from the project (the graph panel's Clear all channels). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::graph_keys::tool_graph_clear_channels,
        },
        // ---- Wave 11 integrator-authored entries ----
        Tool {
            name: "graph_paste_keys_at",
            description: "Paste keys into a graph channel addressed by its index in graph_list_channels (the by-index twin of graph_paste_keys — use it when two channels share a figure+target and the name form would resolve the first). The earliest pasted key lands at 'at_frame'; existing keys within half a frame of a pasted key are replaced. 'keys' is the array graph_copy_keys returns. One undo step.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "keys": { "type": "array", "items": { "type": "object" }, "description": "Key objects as returned by graph_copy_keys" },
                    "at_frame": { "type": "number" }
                }),
                &["channel", "keys", "at_frame"],
            ),
            handler: tool_graph_paste_keys_at,
        },
        Tool {
            name: "fx_preset_delete",
            description: "Delete a saved FX clip preset by name (the file in ./fx-presets/). A missing preset is an error naming it. A file operation, not an undo step.",
            schema: obj_schema(
                json!({ "name": { "type": "string" } }),
                &["name"],
            ),
            handler: crate::fx_ops::tool_fx_preset_delete,
        },
        Tool {
            name: "figure_cycle_opacity",
            description: "Cycle a figure's opacity one step through the inspector's steps (1.0 -> 0.6 -> 0.3 -> 1.0). 'figure' defaults to the selected figure. Like the panel cycler, this takes NO undo snapshot.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &[],
            ),
            handler: crate::figure_ops::tool_figure_cycle_opacity,
        },
        Tool {
            name: "get_figure",
            description: "Read one figure in full: name, colour (0xRRGGBB), hidden/locked flags, opacity, node count, movieclip link, solo state, and the drawn bounding box on the current frame (null when the figure has no pose there). Read-only.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &["figure"],
            ),
            handler: crate::figure_ops::tool_get_figure,
        },
        Tool {
            name: "get_node",
            description: "Read one node in full: definition (name, parent, length, thickness, colour override, shape, head flag), the segment style (colour override + effective colour, color2, cap, taper, opacity), the segment's attached media ('media': null, or {kind: 'image', asset, offset, scale, angle_offset, flip} / {kind: 'text', content, size}), and pose facts at 'frame' (default: current) — the stored parent-relative angle in degrees, the solved world direction of the bone ending at this joint in degrees [0, 360) (null for a root node), and the frame's bend in radians and degrees. Note: get_pose returns stored parent-relative angles; the segment inspector shows this same world direction. Read-only.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "frame": { "type": "integer", "description": "Frame to read the pose at; defaults to the current frame" }
                }),
                &["figure", "node"],
            ),
            handler: crate::figure_ops::tool_get_node,
        },
        Tool {
            name: "figure_library_list",
            description: "List the 2D figure library (definitions saved in ./library): index, name and node count per entry, plus the session's current library index. The library stores FigureDef only — an entry's colour is NOT stored; inserting assigns a palette colour. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::figure_ops::tool_figure_library_list,
        },
        Tool {
            name: "figure_library_save",
            description: "Save a figure's definition to the 2D figure library under the figure's name (the library panel's Save). 'figure' defaults to the selected figure and selects it first. A file write, not an undo step.",
            schema: obj_schema(
                json!({ "figure": { "type": "integer" } }),
                &[],
            ),
            handler: crate::figure_ops::tool_figure_library_save,
        },
        Tool {
            name: "figure_library_insert",
            description: "Insert a figure from the 2D figure library into the project (the library panel's Insert). Choose the entry with 'index' or 'name'; with neither, the session's current library index is used. The inserted figure gets a palette colour (the library stores the definition — including its node colours and styles — not the instance colour) and becomes the selection. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &[],
            ),
            handler: crate::figure_ops::tool_figure_library_insert,
        },
        Tool {
            name: "figure_library_cycle",
            description: "Cycle the 2D figure library selection to the next entry (the library panel's cycle). Selection only — inserts nothing. Errors when the library is empty.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::figure_ops::tool_figure_library_cycle,
        },
        Tool {
            name: "fig3_set_camera",
            description: "Set the 3D session camera absolutely: any of 'yaw', 'pitch' (clamped to +-89.9), 'distance' (clamped to 40..=4000), 'target' [x, y, z] and 'fov_deg'; omitted fields keep their values. At least one field is required. The absolute complement of fig3_orbit/fig3_camera_zoom. No undo step (session camera, like orbit/zoom). Requires an active 3D session.",
            schema: obj_schema(
                json!({
                    "yaw": { "type": "number" },
                    "pitch": { "type": "number" },
                    "distance": { "type": "number" },
                    "target": { "type": "array", "items": { "type": "number" }, "description": "[x, y, z] look-at point" },
                    "fov_deg": { "type": "number" }
                }),
                &[],
            ),
            handler: crate::fig3d::tool_fig3_set_camera,
        },
        Tool {
            name: "fig3_rename",
            description: "Rename the 3D session figure — the name a later fig3_save_to_library files it under. Trimmed; a blank name is refused. A session field write, not an undo step. Requires an active 3D session.",
            schema: obj_schema(
                json!({ "name": { "type": "string" } }),
                &["name"],
            ),
            handler: crate::fig3d::tool_fig3_rename,
        },
        Tool {
            name: "clip_delete",
            description: "Delete a movieclip from the project (the clips panel's Delete; figures linked to it are unlinked and an edit session on it ends). One undo step.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: tool_clip_delete,
        },
        Tool {
            name: "clip_rename",
            description: "Rename a movieclip (the clips panel's rename). One undo step when the name actually changes — the op snapshots only on a real change, and an active edit session on the clip follows the rename. Renaming to the current name (or a blank name, which keeps the current one) is a no-op reported with changed=false.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &["index", "name"],
            ),
            handler: tool_clip_rename,
        },
        Tool {
            name: "clip_duplicate",
            description: "Duplicate a movieclip (deep copy, inserted after the original) and return the new clip's index. One undo step.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: tool_clip_duplicate,
        },
        Tool {
            name: "clip_move",
            description: "Move a movieclip one slot 'up' or 'down' in the clip list (the clips panel's reorder). One undo step; moving past either end is an error.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "direction": { "type": "string", "enum": ["up", "down"] }
                }),
                &["index", "direction"],
            ),
            handler: tool_clip_move,
        },
        Tool {
            name: "clip_insert_figure",
            description: "Insert a NEW figure instance of a movieclip (the clips panel's Insert figure): built from the clip's definition, placed at the panel's standard spawn position, linked to the clip, and selected. 'index' is the CLIP index. One undo step.",
            schema: obj_schema(
                json!({ "index": { "type": "integer", "description": "The movieclip's index" } }),
                &["index"],
            ),
            handler: tool_clip_insert_figure,
        },
        Tool {
            name: "clip_enter",
            description: "Enter a movieclip edit session (the clips panel's Edit): subsequent clip_* session tools pose the clip's own frames. Session state only, not an undo step.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: tool_clip_enter,
        },
        Tool {
            name: "clip_exit",
            description: "Exit the current movieclip edit session. A no-op when no session is active. Session state only.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_exit,
        },
        Tool {
            name: "clip_goto",
            description: "Move the movieclip edit session's playhead to a frame of the clip (clamped to the clip's range). Requires an active session (clip_enter).",
            schema: obj_schema(
                json!({ "frame": { "type": "integer" } }),
                &["frame"],
            ),
            handler: tool_clip_goto,
        },
        Tool {
            name: "clip_add_frame",
            description: "Append a frame to the edited movieclip, copying the session's current frame pose. One undo step. Requires an active session (clip_enter).",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_add_frame,
        },
        Tool {
            name: "clip_delete_frame",
            description: "Delete the movieclip edit session's current frame. One undo step. Refuses the clip's last remaining frame. Requires an active session (clip_enter).",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_delete_frame,
        },
        Tool {
            name: "clip_set_angle",
            description: "Set one joint angle (degrees, stored parent-relative) on the movieclip session's current frame. Like the core pose-tweak ops this takes NO undo snapshot (per-angle tweaks would flood the history); use clip_add_frame/undo boundaries deliberately. Requires an active session (clip_enter).",
            schema: obj_schema(
                json!({
                    "node": { "type": "integer" },
                    "degrees": { "type": "number" }
                }),
                &["node", "degrees"],
            ),
            handler: tool_clip_set_angle,
        },
        Tool {
            name: "clip_set_root",
            description: "Set the root position [x, y] of the movieclip session's current frame. No undo snapshot (pose-tweak rule, as clip_set_angle). Requires an active session (clip_enter).",
            schema: obj_schema(
                json!({ "root": { "type": "array", "items": { "type": "number" }, "description": "[x, y] stage position" } }),
                &["root"],
            ),
            handler: tool_clip_set_root,
        },
        Tool {
            name: "clip_get_pose",
            description: "Read the movieclip edit session's current frame: clip index and name, frame, root [x, y] and all joint angles (degrees, parent-relative). Read-only. Requires an active session (clip_enter).",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_get_pose,
        },
        Tool {
            name: "set_project_background",
            description: "Set the project's stage background colour (0xRRGGBB integer). Read it back via get_state's 'background'. One undo step.",
            schema: obj_schema(
                json!({ "color": { "type": "integer", "description": "Colour as a 0xRRGGBB integer" } }),
                &["color"],
            ),
            handler: tool_set_project_background,
        },
        Tool {
            name: "list_projects",
            description: "List the saved projects in ./projects (the Home panel's scan): name, path, figure and frame counts, fps and last-modified time per project. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_list_projects,
        },
        Tool {
            name: "save_project_copy",
            description: "Save a copy of the current project in ./projects (the projects panel's Save a copy): the copy's name is derived — '<project name> copy' — and stored in the copy. Create-new semantics: if that copy already exists the call FAILS rather than overwriting or suffix-walking (a second call therefore refuses, exactly like a second panel press). The session keeps editing the original project.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_save_project_copy,
        },
        Tool {
            name: "duplicate_project",
            description: "Duplicate the current project's saved FILE on disk, walking the -copy / -copy-2 suffixes until a free name is found (the projects panel's Duplicate). The session keeps editing the original. Errors when the current project has no saved file.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_duplicate_project,
        },
        Tool {
            name: "project_rename",
            description: "Rename the current project's saved file to 'new_name' (a target that already exists is refused, leaving both files untouched). The session project's name follows and the file selector is repointed at the renamed file; the name stored inside the file updates on the next save. Errors when the current project has no saved file.",
            schema: obj_schema(
                json!({ "new_name": { "type": "string" } }),
                &["new_name"],
            ),
            handler: tool_project_rename,
        },
        Tool {
            name: "attach_image_to_node",
            description: "Attach a sprite image to the SELECTED joint of the selected figure (the segment inspector's Attach image). Selection-scoped: steer the selection with set_selection first. The op takes the first PNG found in ./import — there is no path argument, matching the GUI op exactly. One undo step (inside the op).",
            schema: obj_schema(json!({}), &[]),
            handler: tool_attach_image_to_node,
        },
        Tool {
            name: "clear_node_media",
            description: "Clear the media (sprite) attached to the SELECTED joint of the selected figure (the segment inspector's Clear media). Selection-scoped: steer the selection with set_selection first. One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clear_node_media,
        },
        Tool {
            name: "assets_list",
            description: "List the project's image asset table: for each asset its index, name, stored byte size, decoded pixel width/height (null when the stored bytes do not decode) and live reference counts split by kind (figure segment images, movieclip-def segment images, sprite instances). Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::media::tool_assets_list,
        },
        Tool {
            name: "asset_info",
            description: "One asset's assets_list entry, addressed by its table index. Read-only.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: crate::media::tool_asset_info,
        },
        Tool {
            name: "asset_import",
            description: "Import a PNG or JPEG file from disk into the asset table (appended; its index is returned). The file must start with the PNG magic bytes or the JPEG SOI marker, decode as an image and be at most 8388608 bytes (8 MiB) — anything else is refused before any change. The stored name defaults to the file's name; the optional 'name' argument overrides it. One undo step. (Unlike attach_image_to_node, this never touches ./import and attaches nothing.)",
            schema: obj_schema(
                json!({
                    "path": { "type": "string" },
                    "name": { "type": "string" }
                }),
                &["path"],
            ),
            handler: crate::media::tool_asset_import,
        },
        Tool {
            name: "asset_rename",
            description: "Rename the asset at 'index' to 'name' (trimmed; empty is refused). Renaming to the current name is a no-op that reports changed: false and spends no undo step; a real rename is one undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" },
                    "name": { "type": "string" }
                }),
                &["index", "name"],
            ),
            handler: crate::media::tool_asset_rename,
        },
        Tool {
            name: "asset_remove",
            description: "Remove the asset at 'index' from the table. REFUSED, with the live counts, while anything still references it (a figure segment image, a movieclip-def segment image, or a sprite instance) — removal never orphans a reference; clear segment references first with clear_node_media or re-point them with asset_attach, and remove sprite instances with sprite_remove. When it does remove, every reference to a higher index is remapped down by one in the same undo step, so references keep naming the same image. One undo step.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: crate::media::tool_asset_remove,
        },
        Tool {
            name: "asset_attach",
            description: "Attach the EXISTING asset at index 'asset' to figure 'figure', node 'node' as that segment's image media, replacing whatever media the segment carried (the by-index counterpart of attach_image_to_node, which reads ./import and works on the selection). One undo step. Optional 'offset' ([along, across] stage units in the segment's own frame), 'scale' (a positive multiplier on the fitted size), 'angle_offset' (degrees) and 'flip' (boolean) set the sprite's local transform; unnamed components take the identity, and re-attaching the same asset with new transform values is how an attached sprite's transform is set. When any transform argument is given, the reply's 'media' names the resulting media in clip_get_node_media's shape.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "asset": { "type": "integer" },
                    "offset": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
                    "scale": { "type": "number" },
                    "angle_offset": { "type": "number" },
                    "flip": { "type": "boolean" }
                }),
                &["figure", "node", "asset"],
            ),
            handler: crate::media::tool_asset_attach,
        },
        Tool {
            name: "sprite_list",
            description: "Every sprite instance in the project: index, asset index and asset name, key count, and every transform key (frame, x, y, scale, rotation in degrees, opacity, flip_x, flip_y, scale_y, pivot_x, pivot_y). Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: crate::media::tool_sprite_list,
        },
        Tool {
            name: "sprite_add",
            description: "Place a new sprite instance of the image asset at 'asset': one key at frame 0 puts it at the stage centre, scale 1, no rotation. Returns the new instance's index. One undo step.",
            schema: obj_schema(
                json!({ "asset": { "type": "integer" } }),
                &["asset"],
            ),
            handler: crate::media::tool_sprite_add,
        },
        Tool {
            name: "sprite_remove",
            description: "Remove the sprite instance at 'sprite'. Later instances shift down one index; nothing references an instance by index, so no remapping is owed. One undo step.",
            schema: obj_schema(
                json!({ "sprite": { "type": "integer" } }),
                &["sprite"],
            ),
            handler: crate::media::tool_sprite_remove,
        },
        Tool {
            name: "sprite_set_key",
            description: "Write sprite 'sprite''s transform key at 'frame' (0-based, a whole frame number; clamped to the project's last frame): insert a new key or replace the one there. 'x', 'y', 'rotation' (degrees), 'scale', 'opacity' (0 to 1), 'flip_x', 'flip_y', 'scale_y' and 'pivot_x'/'pivot_y' (the anchor point, image-relative, 0.5 = centre) are each optional — an omitted field takes the value the sprite samples at that frame (or x 0, y 0, rotation 0, scale 1, opacity 1, no flips, scale_y following scale, centre pivot for a sprite with no keys). 'scale' and 'scale_y' must be greater than 0. One undo step.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "frame": { "type": "number" },
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "scale": { "type": "number" },
                    "rotation": { "type": "number" },
                    "opacity": { "type": "number" },
                    "flip_x": { "type": "boolean" },
                    "flip_y": { "type": "boolean" },
                    "scale_y": { "type": "number" },
                    "pivot_x": { "type": "number" },
                    "pivot_y": { "type": "number" }
                }),
                &["sprite", "frame"],
            ),
            handler: crate::media::tool_sprite_set_key,
        },
        Tool {
            name: "sprite_delete_key",
            description: "Delete sprite 'sprite''s key at 'frame' (0-based, a whole frame number). A frame at/past the project's frame count is refused, never clamped onto the last frame; a frame with no key is refused before any change, so no undo step is spent. Deleting the last remaining key is allowed — the instance then draws nothing. A stale key past the end is deleted by exact frame via sprite_delete_key_at. One undo step on success.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "frame": { "type": "number" }
                }),
                &["sprite", "frame"],
            ),
            handler: crate::media::tool_sprite_delete_key,
        },
        Tool {
            name: "sprite_move_key",
            description: "Move (retime) sprite 'sprite''s transform key from 'from_frame' to 'to_frame' by EXACT frame. The source frame is addressed exactly (never clamped), so a key left past the timeline end by a shortened timeline can be rescued back into range; the destination must be inside the timeline — a past-the-end destination is refused, never clamped. The stored transform travels verbatim (including a scale of 0 or less, which sprite_set_key refuses to write) and a key already at the destination is replaced. Moving a key onto its own frame changes nothing and spends no undo step; a real move is one undo step.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "from_frame": { "type": "integer" },
                    "to_frame": { "type": "integer" }
                }),
                &["sprite", "from_frame", "to_frame"],
            ),
            handler: crate::mcp_sprite_tools::tool_sprite_move_key,
        },
        Tool {
            name: "sprite_delete_key_at",
            description: "Delete sprite 'sprite''s transform key at EXACTLY 'frame': unlike sprite_delete_key, whose frame must be inside the timeline (a frame at/past the frame count is refused there, never clamped onto the last frame), this deletes the key at the frame named — including a stale key left past the timeline end by a shortened timeline — and never a different frame's key. A frame holding no key is refused before any change, so no undo step is spent. One undo step on success.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "frame": { "type": "integer" }
                }),
                &["sprite", "frame"],
            ),
            handler: crate::mcp_sprite_tools::tool_sprite_delete_key_at,
        },
        Tool {
            name: "sprite_set_asset",
            description: "Re-point sprite 'sprite' at image asset 'asset', keeping every transform key verbatim — the fix for a wrong image, which otherwise means removing the instance and re-keying everything. Naming the asset the sprite already uses changes nothing and spends no undo step. One undo step on a real change.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "asset": { "type": "integer" }
                }),
                &["sprite", "asset"],
            ),
            handler: crate::mcp_sprite_tools::tool_sprite_set_asset,
        },
        Tool {
            name: "sprite_reorder",
            description: "Move sprite 'sprite' to 0-based position 'to' in the sprite table — draw order IS table order (later sprites paint over earlier ones), so this is the z-order write. The instances between shift by one, keeping their relative order; the moved sprite keeps its asset and keys verbatim. Naming the position the sprite already holds changes nothing and spends no undo step. One undo step on a real move.",
            schema: obj_schema(
                json!({
                    "sprite": { "type": "integer" },
                    "to": { "type": "integer" }
                }),
                &["sprite", "to"],
            ),
            handler: crate::mcp_sprite_tools::tool_sprite_reorder,
        },
        Tool {
            name: "import_layered_set",
            description: "Import a layered set into the current project from the folder at 'path' — a SpriteLoop package folder (holding spriteloop.import.json, as the official Krita/Photoshop exporters write it) or a plain folder of PNG/JPEG images; a manifest in the folder always wins over the loose-images reading. Every part's image is validated through the shared asset gate before anything changes, so one corrupt part refuses the whole import and spends no undo step. Parts are matched by name to nodes of figure 'figure' (the selected figure when 'figure' is omitted): a matched part dresses its node as segment media, and every unmatched or invisible part becomes a free sprite — no part is ever dropped. The whole import is one undo step.",
            schema: obj_schema(
                json!({
                    "path": { "type": "string" },
                    "figure": { "type": "integer" }
                }),
                &["path"],
            ),
            handler: crate::mcp_layered_tools::tool_import_layered_set,
        },
        // ---------------------------------------------------------
        // Wave 14 — parity fill (Squad A + clip media + transport):
        // every entry below was an unwired handler until this wave;
        // each handler's own TOOL ENTRY SPEC is its provenance.
        // ---------------------------------------------------------
Tool {
      name: "audio_clip_gain_step",
      description: "Step the audio clip at 'index' gain by 'delta' (the audio panel's −/+ gain buttons): the stored gain is clamped to 0.0-2.0 by the op and the reply reports the gain read back after clamping. The relative form of audio_clip_set_gain. One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "delta": { "type": "number" }
          }),
          &["index", "delta"],
      ),
      handler: crate::audio_ops::tool_audio_clip_gain_step,
  },
Tool {
      name: "get_brush",
      description: "Read the ACTIVE brush preset in full (the Draw panel's preset readouts, as data): index, name, width, nib, nib angle, smoothing, opacity and scatter, plus the active draw tool ('brush' or 'eraser'), the eraser radius the preset sizes (twice its width) and the draw-mode flag. Read-only.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::draw_ops::tool_get_brush,
  },
Tool {
      name: "set_draw_mode",
      description: "Set draw mode on or off (the Draw panel's Draw toggle / the palette's draw-mode command, as an idempotent set): in draw mode, dragging on the canvas paints a stroke on the current frame instead of posing. Session state only — not an undo step.",
      schema: obj_schema(
          json!({
              "enabled": { "type": "boolean" }
          }),
          &["enabled"],
      ),
      handler: crate::draw_ops::tool_set_draw_mode,
  },
Tool {
      name: "step_brush_param",
      description: "Step the active brush selection by one preset parameter (the stroke inspector's Preset size / opacity / scatter / smoothing steppers): selects the nearest library preset whose value for 'param' is strictly higher ('direction' 1) or lower (-1). Because presets are bundles the other parameters follow the chosen preset. Reports changed: false, with the panel's status text, when no preset steps that way. Session state only — not an undo step.",
      schema: obj_schema(
          json!({
              "param": { "type": "string", "enum": ["size", "width", "opacity", "scatter", "smoothing"] },
              "direction": { "type": "integer", "description": "1 to step up, -1 to step down" }
          }),
          &["param", "direction"],
      ),
      handler: crate::draw_ops::tool_step_brush_param,
  },
Tool {
      name: "set_brush_nib",
      description: "Select the active brush preset by its nib (the stroke inspector's Preset nib segmented control): the first library preset carrying nib 'round', 'square' or 'calligraphy' becomes active. Session state only — not an undo step.",
      schema: obj_schema(
          json!({
              "nib": { "type": "string", "enum": ["round", "square", "calligraphy"] }
          }),
          &["nib"],
      ),
      handler: crate::draw_ops::tool_set_brush_nib,
  },
Tool {
      name: "apply_nib_stroke",
      description: "Give ONE stroke, by its project-wide index (see list_strokes), a nib width profile: \"round\" (uniform), \"square\" or \"calligraphy\" (angle-dependent, angle in degrees, default 45) — the single-stroke form of apply_nib, computed from the stroke's own width as the nib size. Replaces any per-point widths the stroke carried. One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "nib": { "type": "string", "enum": ["round", "square", "calligraphy"] },
              "angle": { "type": "number", "description": "Nib angle in degrees (default 45)" }
          }),
          &["index", "nib"],
      ),
      handler: crate::draw_ops::tool_apply_nib_stroke,
  },
Tool {
      name: "smooth_stroke",
      description: "Chaikin-smooth ONE stroke by its project-wide index (see list_strokes) — the single-stroke form of smooth_strokes: 'subdiv' corner-cutting passes (default 1, at most 4), endpoints pinned, per-point widths resampled onto the new points. Returns smoothed: false — and takes no undo step — when the stroke has fewer than 2 points and cannot change. One undo step when it smooths.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "subdiv": { "type": "integer", "description": "Chaikin passes, 1-4 (default 1)" }
          }),
          &["index"],
      ),
      handler: crate::draw_ops::tool_smooth_stroke,
  },
Tool {
      name: "mirror_all_strokes",
      description: "Mirror EVERY stroke in the project, on all frames, across the vertical line x = 'cx' (default: the stage's vertical centre) — the project-wide form of mirror_strokes. Only points change. Errors on a project with no strokes. One undo step.",
      schema: obj_schema(
          json!({
              "cx": { "type": "number", "description": "Mirror line x in stage coordinates (default: stage centre)" }
          }),
          &[],
      ),
      handler: crate::draw_ops::tool_mirror_all_strokes,
  },
Tool {
      name: "set_stroke_spray",
      description: "Edit one stroke's spray mark by its project-wide index (see list_strokes) — the field set_stroke_style cannot reach: 'radius' (> 0), 'opacity' (0-1) and/or 'seed' set the mark's fields, creating the mark on a plain stroke (defaults: radius 12, opacity 0.5, the deterministic begin_draw seed); 'clear_spray': true removes the mark instead, returning the stroke to a plain polyline. At least one field is required. Clearing a markless stroke reports changed: false and takes no undo step. One undo step when the mark changes.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "radius": { "type": "number", "description": "Spray radius in stage units" },
              "opacity": { "type": "number" },
              "seed": { "type": "integer" },
              "clear_spray": { "type": "boolean", "description": "Remove the spray mark" }
          }),
          &["index"],
      ),
      handler: crate::draw_ops::tool_set_stroke_spray,
  },
Tool {
      name: "clear_strokes_on_frame",
      description: "Remove every stroke on the NAMED frame (the frame-addressed form of clear_frame_strokes, whose core op is current-frame-only): 'frame' is required and may name any frame, including the current one. One undo step when the frame carried strokes.",
      schema: obj_schema(
          json!({
              "frame": { "type": "integer" }
          }),
          &["frame"],
      ),
      handler: crate::draw_ops::tool_clear_strokes_on_frame,
  },
Tool {
      name: "set_weather_kind",
      description: "Set the scene weather kind (the Scene panel's kind picker): switching kinds keeps the tuned amount/wind, and enabling a kind from off applies the panel's per-kind preset. \"off\" turns weather off. A project-settings tweak — no undo step; a real change marks the project dirty, exactly like the panel.",
      schema: obj_schema(
          json!({
              "kind": { "type": "string", "enum": ["rain", "snow", "fireflies", "fog", "storm", "off"] }
          }),
          &["kind"],
      ),
      handler: crate::editor::tool_set_weather_kind,
  },
Tool {
      name: "cycle_weather",
      description: "Cycle the scene weather like the Scene group's Cycle weather button: off -> rain -> snow -> fireflies -> off, applying each kind's preset wholesale (fog and storm are not in the cycle; cycling from either turns weather off). A project-settings tweak — no undo step; a real change marks the project dirty, exactly like the panel.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::editor::tool_cycle_weather,
  },
Tool {
      name: "set_weather_amount",
      description: "Set the weather's particle amount absolutely (the Scene panel's Amount stepper/slider): clamped to 0..=200. Errors when the weather is off — pick a kind first (set_weather_kind). A project-settings tweak — no undo step; a real change marks the project dirty, exactly like the panel's steppers.",
      schema: obj_schema(
          json!({
              "amount": { "type": "number", "description": "Particles per frame at steady state, 0..=200" }
          }),
          &["amount"],
      ),
      handler: crate::editor::tool_set_weather_amount,
  },
Tool {
      name: "set_weather_wind",
      description: "Set the weather's wind (horizontal drift, stage units per second) absolutely (the Scene panel's Wind stepper/slider): clamped to -200..=200. Errors when the weather is off — pick a kind first (set_weather_kind). A project-settings tweak — no undo step; a real change marks the project dirty, exactly like the panel's steppers.",
      schema: obj_schema(
          json!({
              "wind": { "type": "number", "description": "Horizontal drift in stage units per second, -200..=200" }
          }),
          &["wind"],
      ),
      handler: crate::editor::tool_set_weather_wind,
  },
Tool {
      name: "clear_camera_key",
      description: "Remove the camera key at the CURRENT frame without an undo step (the camera panel's 'Clear key (no undo)' button): the removal counts for dirty/autosave but cannot be undone — for the undoable delete at any frame, use delete_camera_key. Errors when the current frame holds no camera key.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::editor::tool_clear_camera_key,
  },
Tool {
      name: "camera_zoom_by",
      description: "Multiply the camera zoom at the current frame by 'factor' (the camera panel's zoom in/out buttons): upserts a camera key at the current frame with the zoom clamped to 0.3..=4.0, preserving the angle in effect. One undo step.",
      schema: obj_schema(
          json!({
              "factor": { "type": "number", "description": "Zoom multiplier (must be positive; 2.0 doubles, 0.5 halves)" }
          }),
          &["factor"],
      ),
      handler: crate::editor::tool_camera_zoom_by,
  },
Tool {
      name: "rotate_camera",
      description: "Rotate the camera at the current frame by 'degrees' (the camera panel's rotate buttons): upserts a camera key carrying the pan/zoom in effect plus the new angle, wrapped to -180..180. One undo step. For an absolute angle at any frame, use set_camera with 'angle'.",
      schema: obj_schema(
          json!({
              "degrees": { "type": "number", "description": "Rotation delta in degrees (positive = clockwise on stage)" }
          }),
          &["degrees"],
      ),
      handler: crate::editor::tool_rotate_camera,
  },
Tool {
      name: "get_onion_skin",
      description: "Read the onion-skin master switch (the Onion panel's On/Off toggle): 'enabled' is the stage's master switch. The per-side depths are GUI shell state and are NOT reported here — over the live endpoint they are Squad B shell state; 'legacy_prev_frames' is the core's legacy single-side count, which the stage no longer reads. Read-only.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::editor::tool_get_onion_skin,
  },
Tool {
      name: "set_onion_skin",
      description: "Turn the onion-skin master switch on or off (the Onion panel's On/Off toggle). Session view state, not project data: no undo step and no dirty mark, exactly like the panel toggle. The per-side depths are GUI shell state and cannot be set from the core surface.",
      schema: obj_schema(
          json!({
              "enabled": { "type": "boolean", "description": "True to show onion skins, false to hide them" }
          }),
          &["enabled"],
      ),
      handler: crate::editor::tool_set_onion_skin,
  },
Tool {
      name: "get_editor_settings",
      description: "Read the editor-settings surface in one call: weather (kind/amount/wind, or \"off\"), background colour (0xRRGGBB), project easing, tween playback, camera follow target, loop range, onion master switch, fps, stage size and head-ring scale. The consolidated read behind the set_weather_kind / set_project_easing / set_project_background family — several of these values previously had no read at all (project easing, tween playback, head-ring scale). Read-only.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::editor::tool_get_editor_settings,
  },
Tool {
      name: "get_clipboard_state",
      description: "Read what the frame clipboards hold: 'frame' is null when the single-frame clipboard is empty, else the copied frame's figure count and duration; 'range_frames' is the range clipboard's frame count. A paste succeeds only against a non-empty matching clipboard, and refuses when the rig changed since the copy. Read-only; no undo step.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::editor::tool_clipboard_state,
  },
Tool {
      name: "fig3_set_rotation",
      description: "Set a 3D joint's Euler rotation
      absolutely, in degrees: 'joint' (defaults to the
      selected joint) and any of 'x', 'y', 'z'; omitted
      axes keep their values. At least one axis is
      required. The absolute complement of fig3_rotate.
      No undo step (session pose, like fig3_rotate).
      Requires an active 3D session.",
      schema: obj_schema(
          json!({
              "joint": { "type": "integer", "description": "Joint index; defaults to the selected joint" },
              "x": { "type": "number" },
              "y": { "type": "number" },
              "z": { "type": "number" }
          }),
          &[],
      ),
      handler: crate::fig3d::tool_fig3_set_rotation,
  },
Tool {
      name: "fig3_set_root",
      description: "Set the 3D session pose's root
      position absolutely: 'position' [x, y, z] in
      world units (Y up). The write complement of the
      'pose_root' fig3_get reports; bakes project from
      the root, so this is how a baked figure is
      placed. No undo step (session pose). Requires an
      active 3D session.",
      schema: obj_schema(
          json!({
              "position": { "type": "array", "items": { "type": "number" }, "description": "[x, y, z] root position" }
          }),
          &["position"],
      ),
      handler: crate::fig3d::tool_fig3_set_root,
  },
Tool {
      name: "figure_step_opacity",
      description: "Step a figure's opacity by 'delta' (the figures/layers panels' opacity −/+ buttons): the target is rounded to whole percent and clamped to 0.05-1.0 by the op, and the reply reports the opacity read back after clamping. The relative form of the layers tool's opacity. Not an undo step (layers-family panel parity); a real change counts for dirty/autosave.",
      schema: obj_schema(
          json!({
              "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" },
              "delta": { "type": "number", "description": "Opacity step, e.g. 0.1 or -0.1 (non-zero, finite)" }
          }),
          &["delta"],
      ),
      handler: crate::figure_ops::tool_figure_step_opacity,
  },
Tool {
      name: "figure_toggle_hidden",
      description: "Toggle a figure's hidden flag (the figures panel's hide button / the palette's Toggle figure hidden): the relative form of the layers tool's 'hidden'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave.",
      schema: obj_schema(
          json!({
              "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
          }),
          &[],
      ),
      handler: crate::figure_ops::tool_figure_toggle_hidden,
  },
Tool {
      name: "figure_toggle_lock",
      description: "Toggle a figure's locked flag (the figures panel's lock button / the palette's Lock selected figure): the relative form of the layers tool's 'locked'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave.",
      schema: obj_schema(
          json!({
              "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
          }),
          &[],
      ),
      handler: crate::figure_ops::tool_figure_toggle_lock,
  },
Tool {
      name: "figure_toggle_solo",
      description: "Toggle a figure's solo state (the figures panel's solo button / the palette's Solo selected figure): soloing a figure shows it alone; toggling the soloed figure clears solo. The relative form of the layers tool's 'solo'. Session state only — not an undo step and not a project edit.",
      schema: obj_schema(
          json!({
              "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" }
          }),
          &[],
      ),
      handler: crate::figure_ops::tool_figure_toggle_solo,
  },
Tool {
      name: "nudge_path_point",
      description: "Nudge one figure's motion-path point: move the root of its pose at 'frame' by ('dx', 'dy') stage units (the stage's path-point drag, as data; figure_centre/figure_ground compose this primitive for their specific shifts). 'figure' defaults to the selected figure, 'frame' to the current frame. One undo step.",
      schema: obj_schema(
          json!({
              "figure": { "type": "integer", "description": "0-based figure index (default: the selected figure)" },
              "frame": { "type": "integer", "description": "0-based frame index (default: the current frame)" },
              "dx": { "type": "number", "description": "Horizontal shift in stage units (finite)" },
              "dy": { "type": "number", "description": "Vertical shift in stage units (finite)" }
          }),
          &["dx", "dy"],
      ),
      handler: crate::figure_ops::tool_nudge_path_point,
  },
Tool {
      name: "adjust_frame_duration",
      description: "Adjust a frame's duration multiplier by 'factor' (the frame-ops panel's Dur −/+/×0.5/×2 buttons): the stored duration is multiplied and clamped to 0.25-8.0 by the op, and the reply reports the duration read back after clamping. The relative form of set_frame_duration. Applies to the current frame unless 'frame' names another (which moves the playhead there first, the set_frame_easing convention). Not an undo step (timing-tweak panel parity); a real change counts for dirty/autosave.",
      schema: obj_schema(
          json!({
              "factor": { "type": "number", "description": "Duration multiplier (positive, finite); 0.5 halves the hold, 2.0 doubles it" },
              "frame": { "type": "integer", "description": "0-based frame index (default: the current frame)" }
          }),
          &["factor"],
      ),
      handler: crate::frame_swap::tool_adjust_frame_duration,
  },
Tool {
      name: "fx_set_seed",
      description: "Set an FX clip's particle random seed (the value fx_list reports): the same kind with a different seed draws a different particle pattern. One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "seed": { "type": "integer", "description": "Particle random seed (0 to 4294967295)" }
          }),
          &["index", "seed"],
      ),
      handler: crate::fx_ops::tool_fx_set_seed,
  },
Tool {
      name: "fx_set_angle",
      description: "Set an FX clip's emission angle in degrees (the value fx_list reports; the directed kinds — beam, trail, fountain — emit along it). One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "angle": { "type": "number", "description": "Emission angle in degrees" }
          }),
          &["index", "angle"],
      ),
      handler: crate::fx_ops::tool_fx_set_angle,
  },
Tool {
      name: "fx_set_length",
      description: "Set an FX clip's length in stage units — beam reach / trail span (the value add_fx_clip's 'length' sets at creation and fx_list reports). One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "length": { "type": "number", "description": "Beam reach / trail span in stage units" }
          }),
          &["index", "length"],
      ),
      handler: crate::fx_ops::tool_fx_set_length,
  },
Tool {
      name: "sprite_sample",
      description: "Read sprite 'sprite''s sampled transform at 'frame' (fractional frames allowed): the interpolated key between its authored keys, clamped outside the keyed range — the transform the stage and exports draw. Read-only; no undo step. Errors when the sprite has no keys.",
      schema: obj_schema(
          json!({
              "sprite": { "type": "integer" },
              "frame": { "type": "number", "description": "Frame to sample at; may be fractional" }
          }),
          &["sprite", "frame"],
      ),
      handler: crate::media::tool_sprite_sample,
  },
Tool {
      name: "clip_asset_attach",
      description: "Attach the EXISTING asset at index 'asset' to movieclip definition 'clip', node 'node' as that segment's image media, replacing whatever media the definition's segment carried (the clip-def counterpart of asset_attach, which writes a figure's own definition). The media renders on every instance of the clip whose own definition carries no media at that node — an instance's own media wins. One undo step. Optional 'offset', 'scale', 'angle_offset' and 'flip' set the sprite's local transform exactly as on asset_attach; when any is given, the reply's 'media' names the resulting media.",
      schema: obj_schema(
          json!({
              "clip": { "type": "integer" },
              "node": { "type": "integer" },
              "asset": { "type": "integer" },
              "offset": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
              "scale": { "type": "number" },
              "angle_offset": { "type": "number" },
              "flip": { "type": "boolean" }
          }),
          &["clip", "node", "asset"],
      ),
      handler: crate::media::tool_clip_asset_attach,
  },
Tool {
      name: "clip_clear_node_media",
      description: "Clear the media (image or text) attached to movieclip definition 'clip', node 'node' (the clip-def counterpart of clear_node_media, which is selection-scoped to a figure). Clearing a node that carries no media is reported with 'cleared': false, not an error, and spends no undo step; a clear that removes media is one undo step. The reply's 'media' names what was cleared (null when nothing was).",
      schema: obj_schema(
          json!({
              "clip": { "type": "integer" },
              "node": { "type": "integer" }
          }),
          &["clip", "node"],
      ),
      handler: crate::media::tool_clip_clear_node_media,
  },
Tool {
      name: "clip_get_node_media",
      description: "Read the segment media attached to movieclip definition 'clip', node 'node': null, or {kind: 'image', asset, offset, scale, angle_offset, flip} / {kind: 'text', content, size} — the same shape get_node returns for a figure's node. Read-only.",
      schema: obj_schema(
          json!({
              "clip": { "type": "integer" },
              "node": { "type": "integer" }
          }),
          &["clip", "node"],
      ),
      handler: crate::media::tool_clip_get_node_media,
  },
Tool {
      name: "particle_set_seed",
      description: "Set a particle event's random seed (the value particle_list_events reports): the same kind with a different seed draws a different particle pattern. One undo step.",
      schema: obj_schema(
          json!({
              "index": { "type": "integer" },
              "seed": { "type": "integer", "description": "Particle random seed (0 to 4294967295)" }
          }),
          &["index", "seed"],
      ),
      handler: crate::particle_ops::tool_particle_set_seed,
  },
Tool {
      name: "delete_project_file",
      description: "Delete a SAVED project file from ./projects — the Home panel's delete, via MCP. 'path' names the file exactly as list_projects reports it (its full path or its bare file name); anything not in that listing is refused, so the tool cannot be aimed outside the library. The file is removed through the shell's guarded delete (shell-owned files are refused). The currently open project, if this is its file, stays open in memory, exactly as in the panel. No undo step: a file operation, not a project edit (the project_rename / duplicate_project family).",
      schema: obj_schema(
          json!({
              "path": { "type": "string", "description": "Project file to delete, as reported by list_projects (path or bare file name)" }
          }),
          &["path"],
      ),
      handler: crate::projects::tool_delete_project_file,
  },
Tool {
      name: "cycle_project_file",
      description: "Advance the project-file selector by one — the command palette's 'Cycle project file'. Moves the selection in the cached ./projects list (wrapping at the end) and reports the newly selected file; opens nothing. Pair with open_project_file to open the selection, or read the list with list_projects. No undo step: selector state, not a project edit.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::projects::tool_cycle_project_file,
  },
Tool {
      name: "open_project_file",
      description: "Open the project file the selector currently names — the palette's Open project file, companion to cycle_project_file. The selected file replaces the in-memory project (the op's own swap discipline applies, exactly as the GUI's open paths behave; load_project remains the by-path route). A failed open keeps the current project and reports the failure in 'status'. No undo step: the project-swap family, like load_project.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::projects::tool_open_project_file,
  },
Tool {
      name: "shell_play",
      description: "Start playback in the live editor (the Play button): the playhead starts from the current frame and the shell poll advances it and starts playback audio, exactly as for a GUI play press. Idempotent — calling it while already playing changes nothing. Not an undo step; does not mark the project dirty. On the stdio server the flag is set but nothing advances it.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::shell_tools_transport::tool_shell_play,
  },
Tool {
      name: "shell_pause",
      description: "Pause playback in the live editor (the Pause button): the playhead stays exactly where it stopped, and the shell poll stops playback audio. Idempotent — calling it while already paused changes nothing. Not an undo step; does not mark the project dirty.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::shell_tools_transport::tool_shell_pause,
  },
Tool {
      name: "shell_set_playhead",
      description: "Move the playhead to a 0-based frame (a timeline click): sets the current frame and snaps the fractional playhead onto it, playing or paused. Out-of-range frames are refused. The shell-family name for the same move the set_frame built-in makes. Not an undo step; does not mark the project dirty; selection is untouched.",
      schema: obj_schema(
          json!({
              "frame": { "type": "integer" }
          }),
          &["frame"],
      ),
      handler: crate::shell_tools_transport::tool_shell_set_playhead,
  },
Tool {
      name: "shell_get_transport",
      description: "Read the playback transport state: playing, current frame, fractional playhead, frame count, project fps and the loop range ([start, end] inclusive, or null). Read-only. The loop on/off toggle and playback speed are shell state and are not reported here.",
      schema: obj_schema(json!({}), &[]),
      handler: crate::shell_tools_transport::tool_shell_get_transport,
  },
Tool {
            name: "graph_key_interp",
            description: "Read the interpolation mode of the key at 'frame' on the channel at 'channel' index — the graph panel's interp-control readout (the nearest key within 0.5 frames). The reply's 'interp' is \"hermite\"/\"linear\"/\"step\", or null with 'keyed': false when no key sits at that frame. Read-only.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "frame": { "type": "number" }
                }),
                &["channel", "frame"],
            ),
            handler: crate::graph_keys::tool_graph_key_interp,
        },
Tool {
            name: "graph_count_keys_in_range",
            description: "Count the keys of the channel at 'channel' index whose frames fall inside [from, to], both ends inclusive — the read-only pre-flight for graph_delete_keys_in_range (addressed by index, so a duplicate channel counts itself). Read-only.",
            schema: obj_schema(
                json!({
                    "channel": { "type": "integer" },
                    "from": { "type": "number" },
                    "to": { "type": "number" }
                }),
                &["channel", "from", "to"],
            ),
            handler: crate::graph_keys::tool_graph_count_keys_in_range,
        },
Tool {
            name: "graph_set_interp_at_playhead",
            description: "Set the interpolation mode of the graph key nearest the current frame on the SELECTED figure's angle channel for the selected joint: \"hermite\", \"linear\" or \"step\" — the graph panel interp control's fallback when no channel is drawn. set_key_interp addresses any key by channel + key index instead. One undo step when it sets.",
            schema: obj_schema(
                json!({
                    "interp": { "type": "string", "enum": ["hermite", "linear", "step"] }
                }),
                &["interp"],
            ),
            handler: crate::graph_keys::tool_graph_set_interp_at_playhead,
        },
        // ---------------------------------------------------------
        // Wave 14 — shell control (Squad B): live endpoint only.
        // These entries' handlers are the shared stubs below
        // (tool_shell_unavailable_*): on stdio they refuse with
        // the live-editor wording; on the live endpoint the
        // drain's peek intercepts the name before the router
        // and the shell drain executes the real handler
        // against the GUI State. tools/list is therefore
        // identical on both transports.
        // ---------------------------------------------------------
Tool {
            name: "shell_set_workspace",
            description: "Switch the running editor's workspace by name: one of Animate, Draw, Graph, Effects, Audio, 3D, Export, Home (exact, case-sensitive; '3D' is the merged Build/3D workspace). The switch is the workspace chip's own action — it re-gates which panels mount and persists to the layout prefs. An unknown name is refused, listing the valid set. Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema( json!({ "workspace": { "type": "string" } }), &["workspace"], ),
            handler: tool_shell_unavailable_set_workspace,
        },
Tool {
            name: "shell_get_state",
            description: "Read the running editor's shell state in one call: the active workspace; the transport (playing or paused, current frame and fractional playhead, frame count, fps, loop range and playback toggles); the view (view-layer zoom and pan, kept separate from the authored camera at the current frame, plus the effective on-screen zoom the two compose to); the selections (figure, node, sprite, audio clip, FX clip, and the movieclip edit session if one is open); the theme; and which collapsible panels are collapsed. Read-only; no undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_get_state,
        },
Tool {
            name: "shell_set_onion_depths",
            description: "Set the onion-skin ghost depths in the live editor — 'prev' and/or 'next' (0-3 each), the Onion section's steppers by target value. Shell view state: no undo step, never marks the project dirty, session-only exactly like the steppers. Live endpoint only.",
            schema: obj_schema(json!({ "prev": { "type": "integer" }, "next": { "type": "integer" } }), &[]),
            handler: tool_shell_unavailable_set_onion_depths,
        },
Tool {
            name: "shell_view_zoom",
            description: "Set the editor's VIEW zoom without touching the authored camera: 'factor' multiplies the current view zoom (the overlay steppers' operation), 'percent' sets the effective on-screen zoom (camera × view, the overlay readout's number) to that percent. Exactly one of the two is required. The view zoom is clamped to the GUI's own 0.1×–8× range and the reply reports 'clamped' when it is. View navigation only: no camera key is written, it is not an undo step and it does not mark the project dirty.",
            schema: obj_schema(json!({ "factor": { "type": "number" }, "percent": { "type": "number" } }), &[]),
            handler: tool_shell_unavailable_view_zoom,
        },
Tool {
            name: "shell_view_fit",
            description: "Fit the stage view to the authored camera's own framing (the stage pill's Fit): resets the view zoom to 1× and the view pan to zero. View navigation only — unlike the Camera panel's Fit it writes no camera key, is not an undo step and does not mark the project dirty.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_view_fit,
        },
Tool {
            name: "shell_view_centre",
            description: "Centre the stage VIEW on the selected figure's root (the stage pill's Centre), panning the view layer only. Optional 'x' and 'y' (stage units, both together) centre on that stage point instead. Writes no camera key, is not an undo step and does not mark the project dirty.",
            schema: obj_schema(json!({ "x": { "type": "number" }, "y": { "type": "number" } }), &[]),
            handler: tool_shell_unavailable_view_centre,
        },
Tool {
            name: "shell_view_reset",
            description: "The stage pill's 1:1: set the VIEW zoom so the effective on-screen zoom (camera × view) is exactly 100%, leaving the view pan unchanged. Despite the name this is not a reset to the camera framing — that is shell_view_fit. Writes no camera key, is not an undo step and does not mark the project dirty.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_view_reset,
        },
Tool {
            name: "shell_get_view",
            description: "Read the shell view state: the view zoom multiplier and pan (canvas points), 'at_camera_framing' (the view is exactly the authored camera framing), the authored camera zoom at the current frame, the effective on-screen zoom (camera × view) as a factor and a percent, and the stage element/stage sizes. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_get_view,
        },
Tool {
            name: "shell_select_figure",
            description: "Select a figure in the GUI exactly as clicking its row in the Figures panel does: 'figure' (0-based) becomes the selected figure and the node selection is cleared. Shell selection only — no undo step, never marks the project dirty.",
            schema: obj_schema(json!({ "figure": { "type": "integer" } }), &["figure"]),
            handler: tool_shell_unavailable_select_figure,
        },
Tool {
            name: "shell_select_node",
            description: "Select node 'node' of figure 'figure' in the GUI exactly as pressing that joint on the stage does: both the figure and the node selection move. Shell selection only — no undo step (the GUI's stage press snapshots for the drag that may follow, not for the selection; see this file's module doc), never marks the project dirty.",
            schema: obj_schema(json!({ "figure": { "type": "integer" }, "node": { "type": "integer" } }), &["figure", "node"]),
            handler: tool_shell_unavailable_select_node,
        },
Tool {
            name: "shell_select_sprite",
            description: "Select sprite instance 'sprite' (0-based) in the GUI exactly as clicking its row in the Sprites panel does. Shell selection only — no undo step, never marks the project dirty.",
            schema: obj_schema(json!({ "sprite": { "type": "integer" } }), &["sprite"]),
            handler: tool_shell_unavailable_select_sprite,
        },
Tool {
            name: "shell_select_fx_clip",
            description: "Select FX clip 'clip' (0-based, an index into the project's FX clips) in the GUI exactly as pressing its bar in the FX lane or its panel row does. Shell selection only — no undo step, never marks the project dirty.",
            schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"]),
            handler: tool_shell_unavailable_select_fx_clip,
        },
Tool {
            name: "shell_select_audio_clip",
            description: "Select audio clip 'clip' (0-based, an index into the project's audio clips) in the GUI exactly as pressing its row in the Audio panel does — the clip the audio operations toolbar targets. Shell selection only — no undo step, never marks the project dirty.",
            schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"]),
            handler: tool_shell_unavailable_select_audio_clip,
        },
Tool {
            name: "shell_select_frame",
            description: "Move the GUI playhead to frame 'frame' (0-based) exactly as a plain click on that frame's timeline chip does: the playhead moves and any timeline range selection is cleared. Shell selection only — no undo step, never marks the project dirty.",
            schema: obj_schema(json!({ "frame": { "type": "integer" } }), &["frame"]),
            handler: tool_shell_unavailable_select_frame,
        },
Tool {
            name: "shell_select_stroke",
            description: "Select a stroke in the GUI — 'stroke' (0-based, an index into the project's stroke list, the index delete_stroke consumes) becomes the shell's stroke selection, the one the Edit menu's Delete stroke acts on. Shell selection only — no undo step, never marks the project dirty. Live endpoint only.",
            schema: obj_schema(json!({ "stroke": { "type": "integer" } }), &["stroke"]),
            handler: tool_shell_unavailable_select_stroke,
        },
Tool {
            name: "shell_select_particle",
            description: "Select a particle event in the GUI — 'particle' (0-based, an index into the project's particle events) becomes the shell's particle selection, the one the Edit menu's Delete particle event acts on. Shell selection only — no undo step, never marks the project dirty. Live endpoint only.",
            schema: obj_schema(json!({ "particle": { "type": "integer" } }), &["particle"]),
            handler: tool_shell_unavailable_select_particle,
        },
Tool {
            name: "shell_select_clip",
            description: "Select a movieclip in the GUI — 'clip' (0-based, an index into the project's clips) becomes the clips panel's selection. Shell selection only — no undo step, never marks the project dirty. Live endpoint only.",
            schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"]),
            handler: tool_shell_unavailable_select_clip,
        },
Tool {
            name: "shell_select_graph_channel",
            description: "Select a graph channel in the GUI — 'channel' (0-based, an index into the project's channels) becomes the graph panel's drawn channel, exactly as pressing that channel's row in the channel list does: the pick is recorded, the panel target syncs to the channel, and if the channel belongs to another figure that figure becomes selected. This is the selection the panel's and palette's drawn-channel operations (delete, duplicate, shift, fill keys) act on — and the only way to point them at the second of two duplicate channels for the same figure and target. Shell selection only — no undo step, never marks the project dirty. Live endpoint only.",
            schema: obj_schema(json!({ "channel": { "type": "integer" } }), &["channel"]),
            handler: tool_shell_unavailable_select_graph_channel,
        },
Tool {
            name: "shell_get_selection",
            description: "Read back every GUI selection: the selected figure and node, the selected sprite, FX clip and audio clip, the playhead frame, and the timeline range selection if one is active, each with the live count it indexes into. Read-only.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_get_selection,
        },
Tool {
            name: "shell_set_theme",
            description: "Set the editor theme: 'dark' or 'light'. Sets the existing theme outright (never a blind toggle) via the same field the Settings card's Dark/Light buttons write; the shell persists it to the layout prefs on the same tick. Applies immediately, no restart. Accent colours are not touched. Live endpoint only — on stdio this name returns the shell-refusal error. No undo step (shell state).",
            schema: obj_schema(json!({ "theme": { "type": "string", "enum": ["dark", "light"] } }), &["theme"]),
            handler: tool_shell_unavailable_set_theme,
        },
Tool {
            name: "shell_get_settings",
            description: "Read the editor's user-facing settings (the Settings card as a tool): theme, live-MCP state with its endpoint and persisted preference, language, the project default easing, the stage-guides flags, loop playback and playback speed, the current and startup workspaces, the stage size, and the autosave status. Read-only. Live endpoint only — on stdio this name returns the shell-refusal error.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_get_settings,
        },
Tool {
            name: "shell_set_setting",
            description: "Set one Settings-card setting by name: 'theme' (\"dark\"/ \"light\"), 'live_mcp' (bool), 'language' (BCP 47 code), 'default_easing' (\"linear\"/\"smooth\"/\"ease-in\"/\"ease-out\"), 'guides', 'camera_frame', 'safe_frames', 'loop_playback' (bools), 'playback_speed' (one of 0.25, 0.5, 1, 1.5, 2). Each persists exactly as the Settings card persists it; guides/loop/speed are session-only, as in the GUI. All apply immediately, no restart — except live_mcp=false, which the shell applies the moment this reply has been delivered, so the reply is never eaten by the shutdown it orders. Names the GUI does not expose as settable (autosave, startup_workspace, stage size, playback_audio) are refused with the reason. Live endpoint only. No undo step.",
            schema: obj_schema(json!({ "setting": { "type": "string" }, "value": {} }), &["setting", "value"]),
            handler: tool_shell_unavailable_set_setting,
        },
Tool {
      name: "shell_set_section",
      description: "Collapse ('collapsed': true) or expand a panel section in the live editor — the panel header's Hide/Show control. Only the Figures and Library sections have a collapse control; any other section is refused by name. A section not shown in the active workspace is refused (sections are workspace-gated). Shell view state only: no undo step, nothing persisted. Live endpoint only.",
      schema: obj_schema(
          json!({
              "section": { "type": "string" },
              "collapsed": { "type": "boolean" }
          }),
          &["section", "collapsed"],
      ),
      handler: tool_shell_unavailable_set_section,
  },
Tool {
      name: "shell_set_column_widths",
      description: "Set the left and/or right panel column widths in points — the draggable splitters, by target width. At least one of 'left' / 'right' is required. Widths persist in the layout prefs exactly as a drag does (load-modify-save) and are clamped to 220-420 (left) / 260-480 (right); the reply reports the widths read back after the save, so a clamped request truthfully reports the clamped value. No undo step. Live endpoint only.",
      schema: obj_schema(
          json!({
              "left": { "type": "number" },
              "right": { "type": "number" }
          }),
          &[],
      ),
      handler: tool_shell_unavailable_set_column_widths,
  },
Tool {
      name: "shell_set_timeline",
      description: "Collapse ('collapsed': true) or expand the timeline in the live editor. The write lands on the shell's own timeline_collapsed state (State.frame_ops), so the timeline card switches between its full form and its frame-chips-only form immediately, and persists through the layout prefs (load-modify-save, as a workspace switch persists), so a restart restores it; the reply echoes the live value and the persisted read-back. No undo step. Live endpoint only.",
      schema: obj_schema(
          json!({
              "collapsed": { "type": "boolean" }
          }),
          &["collapsed"],
      ),
      handler: tool_shell_unavailable_set_timeline,
  },
Tool {
      name: "shell_get_layout",
      description: "Read the live editor layout: the active workspace, its sections in panel order (each with 'collapsible' and, for Figures/Library, 'collapsed'; the rest report null), the sections the workspace hides, the left/right column visibility (show_left / show_right) and widths, the timeline_collapsed state ('timeline_collapse_wired': true — the shell reads and writes it, see shell_set_timeline), and the sidebar_visible pref ('sidebar_pref_wired': true — the left column's persisted mirror: show_left seeds from it at startup and flips persist back). Read-only. Live endpoint only.",
      schema: obj_schema(json!({}), &[]),
      handler: tool_shell_unavailable_get_layout,
  },
Tool {
            name: "shell_get_recent_colours",
            description: "Read the style inspector's Recent colours list — the colours the user has applied to a node from the style panel this session, most recent first (max 8), each as a '#RRGGBB' hex string and the numeric value the colour tools take. Session-only shell state: not persisted, and colours applied through MCP tools are not recorded in it. Read-only. Live endpoint only — on stdio this name returns the shell-refusal error.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_get_recent_colours,
        },
Tool {
            name: "shell_list_recents",
            description: "List the running editor's recent projects — the File menu's Open recent entries, most recent first, at most five: each with its menu index, its display name, the project file it resolves to (exact stored-name match, then file-stem fallback, against the Home library scan) or null with missing: true when the file is gone. Read-only; no undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_list_recents,
        },
Tool {
            name: "shell_open_recent",
            description: "Open a recent project by its File menu index (0-based, most recent first; at most five entries — see shell_list_recents). The menu's own press, verbatim: the name resolves against the Home library scan (exact stored name, then file stem), the file opens through the shell's open path (selector insert-or-select, then the core open, recents re-touched), and the shell follow-ups run (stale-solo guard, name field re-sync, stroke selection cleared). An entry whose file is gone is refused with the menu's own not-found wording and changes nothing; an out-of-range index is refused, quoting the index and the entry count. Like the menu press, this replaces the session WITHOUT the Home rows' two-press discard confirm — a client that wants the guard checks the dirty state first (get_editor_status). A project swap is not an undo step (the GUI's opens take none). Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema( json!({ "index": { "type": "integer" } }), &["index"], ),
            handler: tool_shell_unavailable_open_recent,
        },
Tool {
            name: "shell_reload_project",
            description: "Reload the current project from its saved file, discarding unsaved changes — the Project section's Reload from disk press, verbatim, including its two-press discard gate: on a dirty session the first call only arms the reload (reply ok: false, armed: true, with the button's own arming wording in status) and the identical second call runs it. The target is the file the shell's last-opened record names for this project; with no saved file the call is refused ('No saved file to reload.'). The reload itself is the core open of that file, with the discard named in the resulting status. A project swap is not an undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_reload_project,
        },
Tool {
            name: "shell_recover_snapshot",
            description: "Recover the autosave snapshot (./projects/.recovery.json) as the current project — the Project section's / Home banner's Recover press, verbatim, including its two-press discard gate: on a dirty session the first call only arms (reply ok: false, armed: true, with the button's own arming wording in status) and the identical second call runs it. With no snapshot on disk the call is refused ('No recovery snapshot found in ./projects/.'). The recovery file is only READ — this tool never writes it. A project swap is not an undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_shell_unavailable_recover_snapshot,
        },
Tool {
            name: "shell_duplicate_project_file",
            description: "Duplicate a saved project file from the Home library — any row, addressed by its stored name (see list_projects): the copy lands beside the source as '<stem>-copy.json', incrementing if taken, exactly as the row's Duplicate button does it, and the session's cached file list is refreshed to name the copy. The current session is untouched (the panel's semantics; distinct from the duplicate_project tool, which duplicates only the current project's file). An unknown name is refused, quoting it. Not an undo step; does not mark the session dirty. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema( json!({ "name": { "type": "string" } }), &["name"], ),
            handler: tool_shell_unavailable_duplicate_project_file,
        },
Tool {
            name: "shell_delete_project_file",
            description: "Delete a saved project file from the Home library, addressed by its stored name (see list_projects) — the row's Delete button and its confirm line, as a two-call protocol over the panel's own arm state: the first call (no 'confirm') only arms the deletion (reply ok: false, armed: true) and touches no file; the second call, with 'confirm': true, carries it out — but only if the arm for THIS name is the one standing, otherwise it is refused. A GUI arm and a tool confirm (or the reverse) interoperate, since both use the panel's arm. Shell-owned files (the recovery snapshot, the recents and layout files) are refused by the shell's own guard. Only the file is removed: deleting the currently open project's file leaves the session's in-memory copy exactly as it is, as in the GUI. An unknown name is refused, quoting it. Not an undo step. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema( json!({ "name": { "type": "string" }, "confirm": { "type": "boolean" }, }), &["name"], ),
            handler: tool_shell_unavailable_delete_project_file,
        },
Tool {
            name: "shell_set_sidebar",
            description: "Show or hide one of the editor's side columns — the View menu's 'Figures sidebar' (side: 'left') and 'Inspector sidebar' (side: 'right') items: flips the same State fields the menu items flip, nothing else; the columns' contents and widths are untouched (widths are B8's shell_set_column_widths; visibility is read back by B8's shell_get_layout). An unknown side is refused, quoting it. Session state only: no undo step, never marks dirty. Live endpoint only — the stdio server refuses this tool.",
            schema: obj_schema( json!({ "side": { "type": "string", "enum": ["left", "right"] }, "visible": { "type": "boolean" }, }), &["side", "visible"], ),
            handler: tool_shell_unavailable_set_sidebar,
        },
    ]
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn call(
        core: &mut EditorCore,
        tools: &[crate::mcp::Tool],
        name: &str,
        args: Value,
    ) -> Result<Value, String> {
        let tool = tools
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("tool {name} not registered"));
        (tool.handler)(core, &args)
    }

    /// Write a real half-second WAV into the temp dir and
    /// return its path: since Wave 11 `add_audio_clip`
    /// validates the file (nonexistent / non-WAV paths are
    /// refused, like the e2e audio suite pins), tests that
    /// place clips must point at a file that exists.
    fn test_wav(tag: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "mcp-tools-test-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = crate::audio_edit::WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: (0..4_000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect(),
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn tool_table_is_complete_and_unique() {
        let tools = tools();
        // 224 entering Wave 12; +6 for the asset surface
        // (assets_list, asset_info, asset_import,
        // asset_rename, asset_remove, asset_attach); +5 in
        // Wave 13 for the sprite surface (sprite_list,
        // sprite_add, sprite_remove, sprite_set_key,
        // sprite_delete_key); +78 in Wave 14 (47 core
        // parity handlers, 31 live-only shell stubs);
        // +4 in the Focused Wave (figure_library_rename
        // plus the shell_select_graph_channel,
        // shell_set_timeline and shell_get_recent_colours
        // stubs); +2 in Wave P (sprite_move_key and
        // sprite_delete_key_at, spliced from the TOOL
        // ENTRY SPECs in src/mcp_sprite_tools.rs).
        // +9 in Wave X: +7 state tools (X3:
        // figure_list_states, figure_set_state,
        // figure_clear_state, figure_assign_clip,
        // figure_unlink_clip, clip_compatible,
        // clip_create_range) and +2 sprite tools (X5:
        // sprite_set_asset, sprite_reorder, spliced from
        // the TOOL ENTRY SPECs in src/mcp_sprite_tools.rs).
        // +2 in Wave Y: import_layered_set (Y6, spliced
        // from the TOOL ENTRY SPEC in
        // src/mcp_layered_tools.rs) and set_rest_pose
        // (Y7's MCP twin, IA-Y7-1).
        assert_eq!(tools.len(), 330);
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 330, "tool names must be unique");
        for t in &tools {
            assert_eq!(t.schema["type"], "object", "tool {} schema", t.name);
        }
    }

    // -----------------------------------------------------------------
    // Wave 13 (B9): the dirty-census contract for the handlers
    // that used to write project state directly — tweak/settings
    // writes count exactly one edit via `mark_dirty` when (and
    // only when) a value actually changes, and take NO undo
    // step; the draw-family rewrite (`apply_nib`) takes exactly
    // one snapshot. `edits_since_autosave` is the observable
    // counter for both (snapshot bumps it too, so the undo
    // assertions separate the two).
    // -----------------------------------------------------------------

    #[test]
    fn set_frame_duration_marks_dirty_only_on_change() {
        let tools = tools();
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        call(&mut core, &tools, "set_frame_duration", json!({ "frame": 0, "duration": 2.0 }))
            .unwrap();
        assert_eq!(core.project.frames[0].duration, 2.0);
        assert_eq!(core.edits_since_autosave(), edits + 1, "a real change counts one edit");
        // Same value again: no change, no mark.
        call(&mut core, &tools, "set_frame_duration", json!({ "frame": 0, "duration": 2.0 }))
            .unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        // No undo step was recorded: undo leaves the value.
        core.undo();
        assert_eq!(core.project.frames[0].duration, 2.0);
    }

    #[test]
    fn set_frame_durations_marks_once_for_the_call() {
        let tools = tools();
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        call(&mut core, &tools, "set_frame_durations", json!({ "durations": [2.0] })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        call(&mut core, &tools, "set_frame_durations", json!({ "durations": [2.0] })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1, "identical values mark nothing");
        call(&mut core, &tools, "set_frame_durations", json!({ "durations": [3.0] })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 2);
    }

    #[test]
    fn set_weather_marks_dirty_on_change_and_on_off() {
        let tools = tools();
        let mut core = EditorCore::new();
        assert!(core.project.weather.is_none());
        let edits = core.edits_since_autosave();
        call(&mut core, &tools, "set_weather", json!({ "kind": "rain" })).unwrap();
        assert!(core.project.weather.is_some());
        assert_eq!(core.edits_since_autosave(), edits + 1);
        // Identical weather (rain at the defaults) marks nothing.
        call(&mut core, &tools, "set_weather", json!({ "kind": "rain" })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        call(&mut core, &tools, "set_weather", json!({ "kind": "off" })).unwrap();
        assert!(core.project.weather.is_none());
        assert_eq!(core.edits_since_autosave(), edits + 2);
        // Off when already off: no change, no mark, and no
        // undo step anywhere in the sequence.
        call(&mut core, &tools, "set_weather", json!({ "kind": "off" })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 2);
        core.undo();
        assert!(core.project.weather.is_none());
    }

    #[test]
    fn set_weather_defaults_match_the_panel_presets() {
        let tools = tools();
        // (kind, amount, wind): the Scene panel presets from
        // editor::eds_kind_preset — the single source the
        // editor-settings handlers and this tool now share
        // (Focused Wave, C1; this tool's own table used to
        // default every non-storm kind to 90/20).
        for (kind, amount, wind) in [
            ("rain", 90.0, 25.0),
            ("snow", 90.0, 12.0),
            ("fireflies", 60.0, 4.0),
            ("fog", 60.0, 0.0),
            ("storm", 140.0, 30.0),
        ] {
            let mut core = EditorCore::new();
            let r = call(&mut core, &tools, "set_weather", json!({ "kind": kind })).unwrap();
            assert_eq!(r["weather"]["amount"], amount, "{kind} default amount");
            assert_eq!(r["weather"]["wind"], wind, "{kind} default wind");
            let w = core.project.weather.unwrap();
            assert_eq!((w.amount, w.wind), (amount, wind), "{kind} stored default");
        }
        // Explicit arguments still win over the preset.
        let mut core = EditorCore::new();
        let r = call(
            &mut core,
            &tools,
            "set_weather",
            json!({ "kind": "rain", "amount": 50.0, "wind": -7.0 }),
        )
        .unwrap();
        assert_eq!(r["weather"]["amount"], 50.0);
        assert_eq!(r["weather"]["wind"], -7.0);
        let w = core.project.weather.unwrap();
        assert_eq!((w.amount, w.wind), (50.0, -7.0));
    }

    #[test]
    fn layers_marks_dirty_once_and_never_snapshots() {
        let tools = tools();
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        // Two project fields in one call = one counted edit.
        call(
            &mut core,
            &tools,
            "layers",
            json!({ "figure": 0, "locked": true, "hidden": true }),
        )
        .unwrap();
        assert!(core.project.figures[0].locked && core.project.figures[0].hidden);
        assert_eq!(core.edits_since_autosave(), edits + 1);
        // Re-applying the same state changes nothing.
        call(
            &mut core,
            &tools,
            "layers",
            json!({ "figure": 0, "locked": true, "hidden": true }),
        )
        .unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        // Solo is session state: no project write, no mark.
        call(&mut core, &tools, "layers", json!({ "figure": 0, "solo": true })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        // Opacity through the clamped setter also counts once.
        call(&mut core, &tools, "layers", json!({ "figure": 0, "opacity": 0.5 })).unwrap();
        assert_eq!(core.project.figures[0].opacity, 0.5);
        assert_eq!(core.edits_since_autosave(), edits + 2);
        // The layers family takes no undo step: undo leaves
        // every write standing.
        core.undo();
        assert!(core.project.figures[0].locked);
        assert_eq!(core.project.figures[0].opacity, 0.5);
    }

    #[test]
    fn apply_nib_takes_exactly_one_undo_step() {
        let tools = tools();
        let mut core = EditorCore::new();
        // Arrange a stroke on frame 0 directly (test setup,
        // not the path under test).
        core.project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [160.0, 120.0], [220.0, 100.0]],
            color: 0xE8E8EC,
            width: 6.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let edits = core.edits_since_autosave();
        let r = call(
            &mut core,
            &tools,
            "apply_nib",
            json!({ "frame": 0, "nib": "calligraphy" }),
        )
        .unwrap();
        assert_eq!(r["strokes"], 1);
        assert!(
            !core.project.strokes[0].widths.is_empty(),
            "the nib baked per-point widths"
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "one snapshot for the call");
        core.undo();
        assert!(
            core.project.strokes[0].widths.is_empty(),
            "one undo restores the pre-nib stroke"
        );
    }

    #[test]
    fn project_rename_marks_dirty_without_an_undo_step() {
        let tools = tools();
        let mut core = EditorCore::new();
        // Arrange a real saved file for the session (the
        // handler renames the file on disk).
        let dir = std::env::temp_dir().join(format!(
            "mcp-tools-test-rename-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("before.json");
        std::fs::write(&path, "{}").unwrap();
        core.project_files = vec![path.clone()];
        core.project_index = 0;
        let edits = core.edits_since_autosave();
        let r = call(
            &mut core,
            &tools,
            "project_rename",
            json!({ "new_name": "Afterwards" }),
        )
        .unwrap();
        assert_eq!(r["name"], "Afterwards");
        assert_eq!(core.project.name, "Afterwards");
        assert_eq!(core.edits_since_autosave(), edits + 1, "the name write counts one edit");
        assert!(dir.join("afterwards.json").exists(), "the file moved");
        // No undo step: undo must not resurrect the old
        // session name against the renamed file.
        core.undo();
        assert_eq!(core.project.name, "Afterwards");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_a_project_through_the_handlers_alone() {
        let tools = tools();
        let mut core = EditorCore::new();

        // New project.
        let r = call(&mut core, &tools, "new_project", json!({ "name": "Agent Film", "fps": 24, "width": 800, "height": 600 }))
            .unwrap();
        assert_eq!(r["project"]["name"], "Agent Film");
        assert_eq!(r["project"]["figures"], 1);
        assert_eq!(r["project"]["frames"], 1);

        // Figures.
        let r = call(&mut core, &tools, "add_figure", json!({ "kind": "quadruped", "name": "Dog", "color": 0xFF8800 })).unwrap();
        assert_eq!(r["figure"], 1);
        let r = call(&mut core, &tools, "add_figure", json!({ "kind": "mini" })).unwrap();
        assert_eq!(r["figure"], 2);
        let r = call(&mut core, &tools, "list_figures", json!({})).unwrap();
        let figs = r["figures"].as_array().unwrap();
        assert_eq!(figs.len(), 3);
        assert_eq!(figs[1]["name"], "Dog");
        assert_eq!(figs[1]["color"], 0xFF8800);
        let humanoid_nodes = figs[0]["nodes"].as_u64().unwrap() as usize;
        assert!(humanoid_nodes > 5);

        // Frames.
        call(&mut core, &tools, "add_frame", json!({})).unwrap();
        call(&mut core, &tools, "add_frame", json!({})).unwrap();
        assert_eq!(core.project.frames.len(), 3);
        call(&mut core, &tools, "set_frame_duration", json!({ "frame": 1, "duration": 2.0 })).unwrap();
        assert_eq!(core.project.frames[1].duration, 2.0);
        core.goto(0);
        call(&mut core, &tools, "insert_inbetween", json!({})).unwrap();
        assert_eq!(core.project.frames.len(), 4);
        call(&mut core, &tools, "delete_frame", json!({ "frame": 3 })).unwrap();
        assert_eq!(core.project.frames.len(), 3);

        // Poses: map form, then full-array form.
        call(&mut core, &tools, "set_pose", json!({
            "figure": 0, "frame": 1, "root": [100.0, 200.0],
            "angles": { "5": -90.0 }
        }))
        .unwrap();
        let pose = call(&mut core, &tools, "get_pose", json!({ "figure": 0, "frame": 1 })).unwrap();
        assert_eq!(pose["root"], json!([100.0, 200.0]));
        assert_eq!(pose["angles"][5], -90.0);
        let zeros = vec![0.0; humanoid_nodes];
        call(&mut core, &tools, "set_pose", json!({ "figure": 0, "frame": 2, "angles": zeros })).unwrap();
        let pose = call(&mut core, &tools, "get_pose", json!({ "figure": 0, "frame": 2 })).unwrap();
        assert_eq!(pose["angles"].as_array().unwrap().len(), humanoid_nodes);
        assert!(pose["angles"].as_array().unwrap().iter().all(|a| *a == 0.0));

        // Camera, FX, weather, audio.
        let r = call(&mut core, &tools, "set_camera", json!({
            "frame": 0, "x": 400.0, "y": 300.0, "zoom": 1.5, "angle": 10.0
        }))
        .unwrap();
        assert_eq!(r["camera"]["angle"], 10.0);
        assert_eq!(core.project.camera.len(), 1);
        let r = call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 6, "kind": "confetti", "start": 1.0, "duration": 12.0
        }))
        .unwrap();
        assert_eq!(r["kind"], "Confetti");
        assert_eq!(core.project.fx_clips.len(), 1);
        call(&mut core, &tools, "set_weather", json!({ "kind": "rain", "amount": 50.0 })).unwrap();
        assert!(matches!(
            core.project.weather.as_ref().map(|w| w.kind),
            Some(WeatherKind::Rain)
        ));
        call(&mut core, &tools, "set_weather", json!({ "kind": "fog" })).unwrap();
        assert!(matches!(
            core.project.weather.as_ref().map(|w| w.kind),
            Some(WeatherKind::Fog)
        ));
        // Storm (wave 12) is heavier by default: amount 140, wind 30.
        call(&mut core, &tools, "set_weather", json!({ "kind": "storm" })).unwrap();
        assert!(matches!(
            core.project.weather.as_ref().map(|w| w.kind),
            Some(WeatherKind::Storm)
        ));
        let w = core.project.weather.unwrap();
        assert_eq!(w.amount, 140.0);
        assert_eq!(w.wind, 30.0);
        let s = call(&mut core, &tools, "get_project_stats", json!({})).unwrap();
        assert_eq!(s["weather"], "storm");
        call(&mut core, &tools, "set_weather", json!({ "kind": "off" })).unwrap();
        assert!(core.project.weather.is_none());
        let music = test_wav("music");
        call(&mut core, &tools, "add_audio_clip", json!({ "path": music, "start_frame": 0 })).unwrap();
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.project.audio[0].start_frame, 0);

        // Graph keys.
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 0.0, "value": 120.0
        }))
        .unwrap();
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": { "angle": 5 }, "frame": 2.0, "value": -45.0
        }))
        .unwrap();
        assert_eq!(core.project.channels.len(), 2);

        // Layers.
        let r = call(&mut core, &tools, "layers", json!({
            "figure": 1, "locked": true, "opacity": 0.5, "solo": true
        }))
        .unwrap();
        assert_eq!(r["locked"], true);
        assert_eq!(r["opacity"], 0.5);
        assert_eq!(core.solo, Some(1));

        // Figure definition edits.
        let nodes_before = core.project.figures[0].def.nodes.len();
        let r = call(&mut core, &tools, "add_node", json!({
            "figure": 0, "parent": 1, "length": 22.0, "thickness": 3.0
        }))
        .unwrap();
        let new_node = r["node"].as_u64().unwrap() as usize;
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before + 1);
        assert_eq!(core.project.figures[0].def.nodes[new_node].length, 22.0);
        // The angle vecs grew to match.
        assert_eq!(core.project.frames[0].poses[0].angles.len(), nodes_before + 1);
        call(&mut core, &tools, "set_node", json!({
            "figure": 0, "node": new_node, "color": 0x00FF00, "shape": "circle"
        }))
        .unwrap();
        assert_eq!(core.project.figures[0].def.nodes[new_node].color, Some(0x00FF00));
        call(&mut core, &tools, "delete_node", json!({ "figure": 0, "node": new_node })).unwrap();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before);

        // Undo/redo handlers respond (the segment ops snapshot).
        assert!(call(&mut core, &tools, "undo", json!({})).is_ok());
        assert!(call(&mut core, &tools, "redo", json!({})).is_ok());
    }

    #[test]
    fn wave7_drawing_fig3_clip_and_bend_tools() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        call(&mut core, &tools, "add_frame", json!({})).unwrap();

        // --- drawing: stroke, nib, smooth, erase ---
        let r = call(&mut core, &tools, "draw_stroke", json!({
            "frame": 0,
            "points": [[100.0, 100.0], [200.0, 100.0], [200.0, 200.0]],
            "color": 0xFF0000,
            "width": 6.0
        }))
        .unwrap();
        assert_eq!(r["strokes_on_frame"], 1);
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].color, 0xFF0000);
        // A second stroke far away, to prove erase is local.
        call(&mut core, &tools, "draw_stroke", json!({
            "frame": 0, "points": [[700.0, 600.0], [760.0, 600.0]]
        }))
        .unwrap();

        let r = call(&mut core, &tools, "apply_nib", json!({
            "frame": 0, "nib": "calligraphy", "angle": 45.0
        }))
        .unwrap();
        assert_eq!(r["strokes"], 2);
        let s0 = &core.project.strokes[0];
        assert_eq!(s0.widths.len(), s0.points.len(), "nib widths stay parallel");
        let (mn, mx) = (
            s0.widths.iter().fold(f32::MAX, |a, &b| a.min(b)),
            s0.widths.iter().fold(f32::MIN, |a, &b| a.max(b)),
        );
        assert!(mx > mn, "calligraphy profile must vary along a bent stroke");

        let before = core.project.strokes[0].points.len();
        let r = call(&mut core, &tools, "smooth_strokes", json!({ "frame": 0 })).unwrap();
        assert_eq!(r["smoothed"], 2);
        assert!(core.project.strokes[0].points.len() > before, "smoothing adds points");
        // Erasing at the first stroke's start removes/changes only it.
        let p0 = core.project.strokes[0].points[0];
        let r = call(&mut core, &tools, "erase_strokes", json!({
            "frame": 0, "x": p0[0], "y": p0[1], "radius": 3.0
        }))
        .unwrap();
        assert_eq!(r["changed"], 1);
        assert_eq!(r["strokes_on_frame"], 2, "the far stroke survives");
        assert_eq!(core.project.strokes[1].points.len(), 4, "far stroke untouched (post-smooth)");

        // --- 3D session: enter, select, rotate, orbit, bake ---
        let r = call(&mut core, &tools, "fig3_enter", json!({})).unwrap();
        assert_eq!(r["active"], true);
        let joints = r["joints"].as_u64().unwrap() as usize;
        assert!(joints > 5);
        let r = call(&mut core, &tools, "fig3_select", json!({ "joint": 2 })).unwrap();
        assert_eq!(r["selected"], 2);
        let r = call(&mut core, &tools, "fig3_rotate", json!({ "axis": 2, "degrees": 30.0 })).unwrap();
        assert_eq!(r["selected"], 2);
        let r = call(&mut core, &tools, "fig3_orbit", json!({ "yaw": 45.0, "pitch": 10.0 })).unwrap();
        assert_eq!(r["camera"]["yaw"], 45.0);
        let figures_before = core.project.figures.len();
        let r = call(&mut core, &tools, "fig3_bake", json!({})).unwrap();
        assert_eq!(core.project.figures.len(), figures_before + 1);
        let baked = r["figure"].as_u64().unwrap() as usize;
        assert!(core.project.figures[baked].name.contains("(3D)"));

        // Animated bake: mark the current pose as B, repose, and
        // interpolate back over 4 frames (timeline grows to fit).
        call(&mut core, &tools, "fig3_mark_b", json!({})).unwrap();
        call(&mut core, &tools, "fig3_rotate", json!({ "axis": 1, "degrees": -60.0 })).unwrap();
        let figures_before = core.project.figures.len();
        let r = call(&mut core, &tools, "fig3_bake_animation", json!({ "frames": 4 })).unwrap();
        assert_eq!(r["frames_baked"], 4);
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert!(core.project.frames.len() >= 4, "timeline extended for the bake");
        assert!(r["name"].as_str().unwrap().contains("(3D anim)"));

        // --- movieclip: create from figure 0, then list ---
        call(&mut core, &tools, "set_pose", json!({
            "figure": 0, "frame": 1, "angles": { "5": -45.0 }
        }))
        .unwrap();
        // The clip copies the figure's poses across the whole
        // timeline — which the animated bake above already grew.
        let timeline = core.project.frames.len();
        let r = call(&mut core, &tools, "clip_create", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["frames"].as_u64().unwrap(), timeline as u64);
        assert_eq!(r["loops_on_figure"], true);
        assert_eq!(core.project.clips.len(), 1);
        assert_eq!(core.project.clips[0].frames[1].angles[5], -45.0);
        let r = call(&mut core, &tools, "clip_list", json!({})).unwrap();
        assert_eq!(r["count"], 1);
        assert_eq!(r["clips"][0]["used_by"], json!([0]));

        // --- bends ---
        let r = call(&mut core, &tools, "set_bend", json!({
            "figure": 0, "frame": 1, "node": 3, "radians": 0.5
        }))
        .unwrap();
        assert_eq!(r["radians"], 0.5);
        assert_eq!(core.project.frames[1].bends[0][3], 0.5);
        assert_eq!(core.project.frames[1].bends[0][2], 0.0, "extended with zeros");
        call(&mut core, &tools, "set_bend", json!({
            "figure": 0, "frame": 1, "node": 3, "radians": 0.0
        }))
        .unwrap();
        assert_eq!(core.project.frames[1].bends[0][3], 0.0);
    }

    #[test]
    fn wave9_stats_and_bulk_durations() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        // A fresh project starts with a single frame; the bulk
        // duration checks below need at least two.
        call(&mut core, &tools, "add_frame", json!({})).unwrap();

        // Stats over the fresh demo project.
        let s = call(&mut core, &tools, "get_project_stats", json!({})).unwrap();
        assert_eq!(s["ok"], json!(true));
        assert!(s["figures"].as_u64().unwrap() >= 1);
        assert!(s["frames"].as_u64().unwrap() >= 1);
        assert!(s["duration_secs"].as_f64().unwrap() > 0.0);
        assert_eq!(s["weather"], Value::Null);
        assert_eq!(s["fx_clips"].as_u64().unwrap(), 0);
        assert_eq!(s["channels"].as_u64().unwrap(), 0);

        // Bulk durations: happy path writes frames 0..n in order.
        let r = call(
            &mut core,
            &tools,
            "set_frame_durations",
            json!({ "durations": [2.0, 0.5] }),
        )
        .unwrap();
        assert_eq!(r["updated"], json!(2));
        assert_eq!(core.project.frames[0].duration, 2.0);
        assert_eq!(core.project.frames[1].duration, 0.5);

        // More entries than frames: only the frame count is applied.
        let n = core.project.frames.len();
        let many: Vec<f64> = vec![1.0; n + 3];
        let r = call(
            &mut core,
            &tools,
            "set_frame_durations",
            json!({ "durations": many }),
        )
        .unwrap();
        assert_eq!(r["updated"], json!(n));

        // A non-positive entry is rejected (mirroring
        // set_frame_duration) and nothing is applied at all.
        core.project.frames[0].duration = 1.0;
        assert!(call(
            &mut core,
            &tools,
            "set_frame_durations",
            json!({ "durations": [3.0, 0.0] })
        )
        .is_err());
        assert_eq!(core.project.frames[0].duration, 1.0);

        // Empty array is a no-op.
        let r = call(
            &mut core,
            &tools,
            "set_frame_durations",
            json!({ "durations": [] }),
        )
        .unwrap();
        assert_eq!(r["updated"], json!(0));
    }

    #[test]
    fn fx_edit_tools_manage_clips() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

        // Every ClipKind the engine renders is addable by name —
        // including the wave-7 kinds (lightning, bubbles, leaves),
        // ember/fountain/orbit, which predate the fx timeline, and
        // the wave-10 kinds (fireworks, aurora, comet) and the
        // wave-11 kinds (galaxy, prism, vortex) and the wave-12
        // kinds (glitter, wave, shard).
        for (name, debug) in [
            ("ember", "Ember"),
            ("fountain", "Fountain"),
            ("orbit", "Orbit"),
            ("lightning", "Lightning"),
            ("bubbles", "Bubbles"),
            ("leaves", "Leaves"),
            ("fireworks", "Fireworks"),
            ("aurora", "Aurora"),
            ("comet", "Comet"),
            ("galaxy", "Galaxy"),
            ("prism", "Prism"),
            ("vortex", "Vortex"),
            ("glitter", "Glitter"),
            ("wave", "Wave"),
            ("shard", "Shard"),
        ] {
            let r = call(&mut core, &tools, "add_fx_clip", json!({
                "figure": 0, "node": 0, "kind": name, "start": 0.0, "duration": 6.0
            }))
            .unwrap();
            assert_eq!(r["kind"], debug, "{name}");
        }
        assert_eq!(core.project.fx_clips.len(), 15);

        // fx_list reports the fx_edit view of the same clips.
        let r = call(&mut core, &tools, "fx_list", json!({})).unwrap();
        assert_eq!(r["count"], 15);
        assert_eq!(r["clips"][3]["kind"], "lightning");
        assert_eq!(r["clips"][3]["figure"], 0);
        assert_eq!(r["clips"][3]["node"], 0);
        assert_eq!(r["clips"][3]["start"], 0.0);
        assert_eq!(r["clips"][3]["end"], 6.0);

        // Move + resize go through the fx_ops EditorCore ops.
        let r = call(&mut core, &tools, "fx_move", json!({ "index": 3, "start": 10.0 })).unwrap();
        assert_eq!(r["start"], 10.0);
        assert_eq!(core.project.fx_clips[3].clip.start, 10.0);
        call(&mut core, &tools, "fx_resize", json!({ "index": 3, "duration": 4.0 })).unwrap();
        assert_eq!(core.project.fx_clips[3].clip.duration, 4.0);

        // Duplicate lands right after the original, in list and time.
        let r = call(&mut core, &tools, "fx_duplicate", json!({ "index": 3 })).unwrap();
        assert_eq!(r["fx_index"], 4);
        assert_eq!(r["start"], 14.0);
        assert_eq!(core.project.fx_clips.len(), 16);
        assert_eq!(core.project.fx_clips[4].clip.start, 14.0);

        // Delete removes exactly the addressed clip.
        let r = call(&mut core, &tools, "fx_delete", json!({ "index": 4 })).unwrap();
        assert_eq!(r["count"], 15);
        assert_eq!(core.project.fx_clips.len(), 15);
        let r = call(&mut core, &tools, "fx_list", json!({})).unwrap();
        assert_eq!(r["count"], 15);
    }

    #[test]
    fn wave10_kinds_parse_and_new_tools_registered() {
        // The wave-10 FX kinds parse by name; near-misses and
        // garbage are rejected with an error, never a panic.
        assert!(matches!(parse_clip_kind("fireworks"), Ok(ClipKind::Fireworks)));
        assert!(matches!(parse_clip_kind("aurora"), Ok(ClipKind::Aurora)));
        assert!(matches!(parse_clip_kind("comet"), Ok(ClipKind::Comet)));
        assert!(parse_clip_kind("firework").is_err());
        assert!(parse_clip_kind("nova").is_err());
        assert!(parse_clip_kind("").is_err());

        // The wave-10 pose/graph tools are in the table and run.
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        let r = call(&mut core, &tools, "mirror_pose", json!({})).unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["status"].is_string());
        let r = call(&mut core, &tools, "smooth_graph_tangents", json!({})).unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["status"].is_string());
    }

    #[test]
    fn wave11_kinds_and_tools() {
        // The wave-11 FX kinds parse by name; near-misses are
        // rejected with an error, never a panic.
        assert!(matches!(parse_clip_kind("galaxy"), Ok(ClipKind::Galaxy)));
        assert!(matches!(parse_clip_kind("prism"), Ok(ClipKind::Prism)));
        assert!(matches!(parse_clip_kind("vortex"), Ok(ClipKind::Vortex)));
        assert!(parse_clip_kind("galax").is_err());
        assert!(parse_clip_kind("prisma").is_err());

        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

        // draw_shape appends one stroke per call on the current
        // frame, with the point count paint::shape_points defines
        // per kind: line 2, rect 5 (closed), ellipse 25.
        let r = call(&mut core, &tools, "draw_shape", json!({
            "kind": "line", "x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 80.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["points"], 2);
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].points.len(), 2);
        assert_eq!(core.project.strokes[0].frame, core.current);
        assert!(core.project.strokes[0].width > 0.0);
        let r = call(&mut core, &tools, "draw_shape", json!({
            "kind": "rect", "x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 80.0
        }))
        .unwrap();
        assert_eq!(r["points"], 5);
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[1].points.len(), 5);
        let r = call(&mut core, &tools, "draw_shape", json!({
            "kind": "ellipse", "x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 80.0
        }))
        .unwrap();
        assert_eq!(r["points"], 25);
        assert_eq!(core.project.strokes.len(), 3);
        assert_eq!(core.project.strokes[2].points.len(), 25);
        // An unknown shape kind errors and adds nothing.
        let r = call(&mut core, &tools, "draw_shape", json!({
            "kind": "circle", "x1": 0.0, "y1": 0.0, "x2": 5.0, "y2": 5.0
        }));
        assert!(r.is_err());
        assert_eq!(core.project.strokes.len(), 3);

        // set_clip_loop cycles the clip's mode Loop -> PingPong ->
        // Once -> Loop (clip_create is how the other tests make a
        // clip).
        call(&mut core, &tools, "clip_create", json!({ "figure": 0 })).unwrap();
        assert_eq!(core.project.clips.len(), 1);
        assert_eq!(core.project.clips[0].loop_mode, crate::model::ClipLoop::Loop);
        let r = call(&mut core, &tools, "set_clip_loop", json!({ "index": 0 })).unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["status"].is_string());
        assert_eq!(core.project.clips[0].loop_mode, crate::model::ClipLoop::PingPong);
        call(&mut core, &tools, "set_clip_loop", json!({ "index": 0 })).unwrap();
        assert_eq!(core.project.clips[0].loop_mode, crate::model::ClipLoop::Once);
        call(&mut core, &tools, "set_clip_loop", json!({ "index": 0 })).unwrap();
        assert_eq!(core.project.clips[0].loop_mode, crate::model::ClipLoop::Loop);
        // Out-of-range is the op's own no-op status, not an error.
        let r = call(&mut core, &tools, "set_clip_loop", json!({ "index": 9 })).unwrap();
        assert_eq!(r["ok"], true);

        // bake_graph on a project with no graph channels is a
        // successful no-op (a status string), not an error.
        let r = call(&mut core, &tools, "bake_graph", json!({ "from": 0, "to": 4 })).unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["status"].is_string());
    }

    #[test]
    fn wave12_kinds_and_tools() {
        // The wave-12 FX kinds parse by name; near-misses are
        // rejected with an error, never a panic.
        assert!(matches!(parse_clip_kind("glitter"), Ok(ClipKind::Glitter)));
        assert!(matches!(parse_clip_kind("wave"), Ok(ClipKind::Wave)));
        assert!(matches!(parse_clip_kind("shard"), Ok(ClipKind::Shard)));
        assert!(parse_clip_kind("gliter").is_err());
        assert!(parse_clip_kind("waves").is_err());

        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        call(&mut core, &tools, "add_frame", json!({})).unwrap();

        // draw_polygon: default 5 sides -> a closed stroke of
        // sides + 1 points on the current frame.
        let r = call(&mut core, &tools, "draw_polygon", json!({
            "cx": 480.0, "cy": 360.0, "radius": 100.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["points"], 6);
        assert_eq!(r["frame"], core.current);
        assert_eq!(core.project.strokes.len(), 1);
        let poly = &core.project.strokes[0];
        assert_eq!(poly.points.len(), 6);
        assert_eq!(poly.points.first(), poly.points.last(), "polygon is closed");
        assert!(poly.width > 0.0);
        // Explicit sides and an explicit target frame.
        let r = call(&mut core, &tools, "draw_polygon", json!({
            "cx": 100.0, "cy": 100.0, "radius": 40.0, "sides": 3, "frame": 0
        }))
        .unwrap();
        assert_eq!(r["points"], 4);
        assert_eq!(r["frame"], 0);
        assert_eq!(core.project.strokes[1].frame, 0);
        // Fewer than 3 sides is an error and adds nothing.
        assert!(call(&mut core, &tools, "draw_polygon", json!({
            "cx": 0.0, "cy": 0.0, "radius": 10.0, "sides": 2
        }))
        .is_err());
        assert_eq!(core.project.strokes.len(), 2);

        // draw_star goes on the current frame (frame 1 after the
        // add_frame above moved the playhead).
        let r = call(&mut core, &tools, "draw_star", json!({
            "cx": 480.0, "cy": 360.0, "radius": 80.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["frame"], core.current);
        assert_eq!(core.project.strokes.len(), 3);
        let star = &core.project.strokes[2];
        assert_eq!(star.frame, core.current);
        assert!(star.points.len() >= 10, "5 spikes alternate outer/inner vertices");
        assert_eq!(r["points"].as_u64().unwrap() as usize, star.points.len());

        // graph_tangent_preset: a bad preset name errors; a real
        // one runs through the editor op's status idiom.
        assert!(call(&mut core, &tools, "graph_tangent_preset", json!({
            "preset": "bouncy"
        }))
        .is_err());
        let r = call(&mut core, &tools, "graph_tangent_preset", json!({
            "preset": "ease-inout"
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["status"].is_string());

        // FX presets: save a clip, add it back at a new frame.
        // A pid-unique name keeps parallel test runs (and stale
        // preset files) from colliding.
        let pname = format!("wave12-test-{}", std::process::id());
        call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 0, "kind": "glitter", "start": 0.0, "duration": 9.0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "fx_preset_save", json!({
            "index": 0, "name": pname
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["kind"], "Glitter");
        let before = core.project.fx_clips.len();
        let r = call(&mut core, &tools, "fx_preset_add", json!({
            "name": pname, "frame": 7.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["kind"], "Glitter");
        assert_eq!(core.project.fx_clips.len(), before + 1);
        let added = &core.project.fx_clips[before];
        assert_eq!(added.clip.kind, ClipKind::Glitter);
        assert_eq!(added.clip.start, 7.0, "the requested frame overrides the stored start");
        assert_eq!(added.clip.duration, 9.0, "the preset's duration is kept");
        // A preset that was never saved is an error, not a panic.
        assert!(call(&mut core, &tools, "fx_preset_add", json!({
            "name": format!("wave12-missing-{}", std::process::id()), "frame": 0.0
        }))
        .is_err());
        assert!(call(&mut core, &tools, "fx_preset_save", json!({
            "index": 99, "name": pname
        }))
        .is_err());
    }

    #[test]
    fn wave13_tools() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        // Grow the timeline to 5 frames for the channel work.
        for _ in 0..4 {
            call(&mut core, &tools, "add_frame", json!({})).unwrap();
        }
        assert_eq!(core.project.frames.len(), 5);

        // --- add_fx_clip color2 stores the gradient end colour ---
        let r = call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 0, "kind": "beam", "start": 0.0, "duration": 6.0,
            "color": 0xFF0000, "color2": 0x0000FF
        }))
        .unwrap();
        assert_eq!(r["color2"], 0x0000FF);
        assert_eq!(core.project.fx_clips[0].clip.color, 0xFF0000);
        assert_eq!(core.project.fx_clips[0].clip.color2, Some(0x0000FF));
        // Without color2 the clip stays a solid colour.
        call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 0, "kind": "burst", "start": 0.0, "duration": 6.0
        }))
        .unwrap();
        assert_eq!(core.project.fx_clips[1].clip.color2, None);

        // --- simplify_strokes reduces a jittery stroke ---
        call(&mut core, &tools, "draw_stroke", json!({
            "frame": 0,
            "points": [[0.0, 0.0], [10.0, 0.4], [20.0, 0.0], [30.0, 0.4], [40.0, 0.0]]
        }))
        .unwrap();
        assert_eq!(core.project.strokes[0].points.len(), 5);
        let r = call(&mut core, &tools, "simplify_strokes", json!({
            "frame": 0, "epsilon": 1.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["reduced"], 1);
        let pts = &core.project.strokes[0].points;
        assert!(pts.len() < 5, "jitter within epsilon is dropped: {pts:?}");
        assert_eq!(pts.first(), Some(&[0.0, 0.0]), "start point kept");
        assert_eq!(pts.last(), Some(&[40.0, 0.0]), "end point kept");
        // A missing epsilon is an error.
        assert!(call(&mut core, &tools, "simplify_strokes", json!({ "frame": 0 })).is_err());

        // --- channel transforms ---
        // A root_x channel with keys at frames 0 and 4.
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 0.0, "value": 0.0
        }))
        .unwrap();
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 4.0, "value": 100.0
        }))
        .unwrap();
        // Scale values x2 around the default pivot 0.
        let r = call(&mut core, &tools, "scale_channel_values", json!({
            "figure": 0, "target": "root_x", "factor": 2.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(core.project.channels[0].keys[0].value, 0.0);
        assert_eq!(core.project.channels[0].keys[1].value, 200.0);
        // Scale around a pivot: v' = 100 + (v - 100) * 0.5.
        call(&mut core, &tools, "scale_channel_values", json!({
            "figure": 0, "target": "root_x", "factor": 0.5, "pivot": 100.0
        }))
        .unwrap();
        assert_eq!(core.project.channels[0].keys[0].value, 50.0);
        assert_eq!(core.project.channels[0].keys[1].value, 150.0);
        // Mirror in time around the default centre (frame 2 of 5):
        // key frames swap 0 <-> 4 and the values travel with them.
        let r = call(&mut core, &tools, "mirror_channel_time", json!({
            "figure": 0, "target": "root_x"
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["centre"], 2.0);
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 2);
        assert_eq!(ch.keys[0].frame, 0.0);
        assert_eq!(ch.keys[0].value, 150.0, "the frame-4 key moved to frame 0");
        assert_eq!(ch.keys[1].frame, 4.0);
        assert_eq!(ch.keys[1].value, 50.0);
        // The "angle:N" string addressing works too.
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "angle:2", "frame": 1.0, "value": 30.0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "mirror_channel_time", json!({
            "figure": 0, "target": "angle:2", "centre": 2.0
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        let ang = core
            .project
            .channels
            .iter()
            .find(|c| matches!(c.target, crate::graph::ChannelTarget::Angle(2)))
            .unwrap();
        assert_eq!(ang.keys[0].frame, 3.0, "frame 1 mirrors to frame 3 around 2");
        // Missing channels error for both transforms.
        assert!(call(&mut core, &tools, "mirror_channel_time", json!({
            "figure": 0, "target": "root_y"
        }))
        .is_err());
        assert!(call(&mut core, &tools, "scale_channel_values", json!({
            "figure": 0, "target": { "angle": 3 }, "factor": 2.0
        }))
        .is_err());
    }

    #[test]
    fn phase_e_op_tools() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

        // --- figure colour + duplicate (figure_ops) ---
        let r = call(&mut core, &tools, "figure_set_color", json!({
            "figure": 0, "color": 0x3366FF
        }))
        .unwrap();
        assert_eq!(r["color"], 0x3366FF);
        assert_eq!(core.project.figures[0].color, 0x3366FF);
        assert!(call(&mut core, &tools, "figure_set_color", json!({ "figure": 0 })).is_err(),
            "the colour argument is required");
        assert!(call(&mut core, &tools, "figure_set_color", json!({
            "figure": 9, "color": 1
        }))
        .is_err());
        let r = call(&mut core, &tools, "figure_duplicate", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["figure"], 1, "the copy lands right after the original");
        assert_eq!(core.project.figures.len(), 2);
        assert_eq!(core.project.figures[1].color, 0x3366FF, "the copy keeps the colour");

        // --- fx colour / colour2 / anchor (fx_ops) ---
        call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 0, "kind": "burst", "start": 0.0, "duration": 6.0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "fx_set_color", json!({
            "index": 0, "color": 0xFF0000
        }))
        .unwrap();
        assert_eq!(r["color"], 0xFF0000);
        assert_eq!(core.project.fx_clips[0].clip.color, 0xFF0000);
        let r = call(&mut core, &tools, "fx_set_color2", json!({
            "index": 0, "color2": 0x0000FF
        }))
        .unwrap();
        assert_eq!(r["color2"], 0x0000FF);
        assert_eq!(core.project.fx_clips[0].clip.color2, Some(0x0000FF));
        // Omitting color2 clears the gradient back to solid.
        let r = call(&mut core, &tools, "fx_set_color2", json!({ "index": 0 })).unwrap();
        assert_eq!(r["color2"], Value::Null);
        assert_eq!(core.project.fx_clips[0].clip.color2, None);
        let r = call(&mut core, &tools, "fx_set_anchor", json!({
            "index": 0, "figure": 1, "node": 2
        }))
        .unwrap();
        assert_eq!(r["figure"], 1);
        assert_eq!(r["node"], 2);
        assert_eq!(core.project.fx_clips[0].figure, 1);
        assert_eq!(core.project.fx_clips[0].node, 2);
        assert!(call(&mut core, &tools, "fx_set_color", json!({
            "index": 7, "color": 1
        }))
        .is_err());
        assert!(call(&mut core, &tools, "fx_set_anchor", json!({
            "index": 0, "figure": 0, "node": 99
        }))
        .is_err());

        // --- delete_stroke removes exactly the addressed stroke ---
        call(&mut core, &tools, "draw_stroke", json!({
            "frame": 0, "points": [[0.0, 0.0], [10.0, 10.0]]
        }))
        .unwrap();
        call(&mut core, &tools, "draw_stroke", json!({
            "frame": 0, "points": [[50.0, 50.0], [60.0, 60.0]]
        }))
        .unwrap();
        assert_eq!(core.project.strokes.len(), 2);
        let r = call(&mut core, &tools, "delete_stroke", json!({ "index": 0 })).unwrap();
        assert_eq!(r["count"], 1);
        assert_eq!(r["frame"], 0);
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(
            core.project.strokes[0].points[0], [50.0, 50.0],
            "the second stroke survives"
        );
        assert!(call(&mut core, &tools, "delete_stroke", json!({ "index": 5 })).is_err());

        // --- audio: *_last tools error cleanly with no clips ---
        assert!(call(&mut core, &tools, "audio_fade", json!({})).is_err());
        assert!(call(&mut core, &tools, "audio_reverse", json!({})).is_err());
        assert!(call(&mut core, &tools, "audio_set_gain", json!({ "gain": 0.5 })).is_err());
        assert!(call(&mut core, &tools, "audio_move_clip", json!({ "start_frame": 3 })).is_err());
        // Move/delete edit the timeline without touching files.
        // (Grow the timeline first so a start frame of 7 is in
        // range even if the op clamps to the timeline.)
        for _ in 0..8 {
            call(&mut core, &tools, "add_frame", json!({})).unwrap();
        }
        let a_wav = test_wav("e-a");
        let b_wav = test_wav("e-b");
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": a_wav, "start_frame": 0
        }))
        .unwrap();
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": b_wav.clone(), "start_frame": 2
        }))
        .unwrap();
        let r = call(&mut core, &tools, "audio_move_clip", json!({ "start_frame": 7 })).unwrap();
        assert_eq!(r["start_frame"], 7);
        assert_eq!(core.project.audio[1].start_frame, 7, "the LAST clip moved");
        assert_eq!(core.project.audio[0].start_frame, 0, "the first clip stayed");
        let r = call(&mut core, &tools, "audio_delete_clip", json!({ "index": 0 })).unwrap();
        assert_eq!(r["count"], 1);
        assert_eq!(core.project.audio[0].path, b_wav);
        assert!(call(&mut core, &tools, "audio_delete_clip", json!({ "index": 4 })).is_err());
    }

    #[test]
    fn phase_e_audio_dsp_tools_run_on_a_real_file() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        // A real half-second WAV on disk: the *_last ops read
        // the clip's file, process it and repoint the clip, so
        // the fake paths the other tests use are not enough.
        let path =
            std::env::temp_dir().join(format!("phase-e-audio-{}.wav", std::process::id()));
        let wav = crate::audio_edit::WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: (0..4_000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect(),
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": path.to_string_lossy().into_owned(), "start_frame": 0
        }))
        .unwrap();
        for name in ["audio_fade", "audio_reverse"] {
            let r = call(&mut core, &tools, name, json!({})).unwrap();
            assert_eq!(r["ok"], true, "{name}");
            assert!(r["status"].is_string(), "{name} reports through the status line");
        }
        let r = call(&mut core, &tools, "audio_set_gain", json!({ "gain": 0.5 })).unwrap();
        assert_eq!(r["ok"], true);
        assert!(call(&mut core, &tools, "audio_set_gain", json!({ "gain": -1.0 })).is_err());
        assert!(call(&mut core, &tools, "audio_set_gain", json!({})).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn phase_i_indexed_audio_dsp_tools_validate_index() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        const NAMES: [&str; 6] = [
            "audio_echo",
            "audio_lowpass",
            "audio_trim_silence",
            "audio_pitch",
            "audio_compress",
            "audio_split_silences",
        ];
        // Missing, non-integer and negative indices are clean
        // errors for every one of the six, clips or no clips.
        for name in NAMES {
            assert!(call(&mut core, &tools, name, json!({})).is_err(), "{name}: missing index");
            assert!(
                call(&mut core, &tools, name, json!({ "index": "0" })).is_err(),
                "{name}: string index"
            );
            assert!(
                call(&mut core, &tools, name, json!({ "index": 1.5 })).is_err(),
                "{name}: fractional index"
            );
            assert!(
                call(&mut core, &tools, name, json!({ "index": -1 })).is_err(),
                "{name}: negative index"
            );
            assert!(
                call(&mut core, &tools, name, json!({ "index": 0 })).is_err(),
                "{name}: no clips yet"
            );
        }
        // With one clip on the timeline, index 1 is out of
        // range: the refusal is the delete_clip error, and the
        // clip list is untouched.
        let a_wav = test_wav("i-a");
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": a_wav, "start_frame": 0
        }))
        .unwrap();
        for name in NAMES {
            let err = call(&mut core, &tools, name, json!({ "index": 1 })).unwrap_err();
            assert_eq!(err, "no audio clip at index 1", "{name}");
        }
        assert_eq!(core.project.audio.len(), 1);
    }

    #[test]
    fn phase_i_indexed_audio_dsp_tools_run_on_real_files() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();
        // Real WAVs on disk: the *_at ops read the addressed
        // clip's file, so the fake paths the other tests use
        // are not enough. Clip 1 is steady tone (for echo);
        // clip 2 is tone, half a second of silence, tone
        // (for the split).
        let dir = std::env::temp_dir();
        let tone_path = dir.join(format!("phase-i-tone-{}.wav", std::process::id()));
        let tone = crate::audio_edit::WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: vec![0.5; 4_000],
        };
        std::fs::write(&tone_path, crate::audio_edit::write_wav(&tone)).unwrap();
        let gap_path = dir.join(format!("phase-i-gap-{}.wav", std::process::id()));
        let mut gap_samples = vec![0.5; 2_000];
        gap_samples.extend(vec![0.0; 4_000]);
        gap_samples.extend(vec![0.5; 2_000]);
        let gap = crate::audio_edit::WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: gap_samples,
        };
        std::fs::write(&gap_path, crate::audio_edit::write_wav(&gap)).unwrap();
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": tone_path.to_string_lossy().into_owned(), "start_frame": 0
        }))
        .unwrap();
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": gap_path.to_string_lossy().into_owned(), "start_frame": 0
        }))
        .unwrap();

        // Echo addresses clip 1 only: it is repointed at its
        // echo product and clip 2 keeps its path.
        let r = call(&mut core, &tools, "audio_echo", json!({ "index": 0 })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["audio_index"], 0);
        assert!(
            r["status"].as_str().unwrap().starts_with("Echo applied to audio clip 1"),
            "status: {}", r["status"]
        );
        assert!(core.project.audio[0].path.ends_with(".echo.wav"));
        assert_eq!(
            core.project.audio[1].path,
            gap_path.to_string_lossy().into_owned()
        );

        // The remaining one-shot effects run against clip 1
        // in turn; each reports through the status line.
        for name in ["audio_lowpass", "audio_trim_silence", "audio_pitch", "audio_compress"] {
            let r = call(&mut core, &tools, name, json!({ "index": 0 })).unwrap();
            assert_eq!(r["ok"], true, "{name}");
            assert_eq!(r["audio_index"], 0, "{name}");
            assert!(r["status"].is_string(), "{name} reports through the status line");
        }

        // Split addresses clip 2: two sounding parts make one
        // added clip, reported in the result.
        let r = call(&mut core, &tools, "audio_split_silences", json!({ "index": 1 })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["audio_index"], 1);
        assert_eq!(r["added"], 1);
        assert_eq!(r["count"], 3);
        assert_eq!(core.project.audio.len(), 3);

        let _ = std::fs::remove_file(&tone_path);
        let _ = std::fs::remove_file(&gap_path);
    }

    #[test]
    fn phase_j_parity_tools_registered_with_right_shapes() {
        let tools = tools();
        // fx_split_clip needs both the clip index and the
        // split point, the latter a number (frame coords
        // are fractional).
        let t = tools
            .iter()
            .find(|t| t.name == "fx_split_clip")
            .expect("fx_split_clip registered");
        assert_eq!(t.schema["required"], json!(["index", "at_frame"]));
        assert_eq!(t.schema["properties"]["index"]["type"], "integer");
        assert_eq!(t.schema["properties"]["at_frame"]["type"], "number");
        // particle_duplicate_event needs just the index.
        let t = tools
            .iter()
            .find(|t| t.name == "particle_duplicate_event")
            .expect("particle_duplicate_event registered");
        assert_eq!(t.schema["required"], json!(["index"]));
        assert_eq!(t.schema["properties"]["index"]["type"], "integer");
        // audio_normalise_all takes no required argument:
        // target_peak is optional with a 0.9 default.
        let t = tools
            .iter()
            .find(|t| t.name == "audio_normalise_all")
            .expect("audio_normalise_all registered");
        assert_eq!(t.schema["required"], json!([]));
        assert_eq!(t.schema["properties"]["target_peak"]["type"], "number");
        // mirror_figure needs just the figure index.
        let t = tools
            .iter()
            .find(|t| t.name == "mirror_figure")
            .expect("mirror_figure registered");
        assert_eq!(t.schema["required"], json!(["figure"]));
        assert_eq!(t.schema["properties"]["figure"]["type"], "integer");
    }

    #[test]
    fn phase_j_indexed_tools_validate_before_the_op() {
        let tools = tools();
        let mut core = EditorCore::new();
        // The demo project has no FX clips and no particle
        // events: index 0 is out of range for both tools,
        // with the pre-check wordings, and a missing index
        // is a clean error too.
        let err = call(&mut core, &tools, "fx_split_clip", json!({ "index": 0, "at_frame": 3.0 }))
            .unwrap_err();
        assert_eq!(err, "no FX clip at index 0");
        assert!(call(&mut core, &tools, "fx_split_clip", json!({ "at_frame": 3.0 })).is_err());
        let err = call(&mut core, &tools, "particle_duplicate_event", json!({ "index": 0 }))
            .unwrap_err();
        assert_eq!(err, "no particle event at index 0");
        assert!(call(&mut core, &tools, "particle_duplicate_event", json!({})).is_err());
        assert!(core.project.fx_clips.is_empty());
        assert!(core.project.particles.is_empty());
    }

    #[test]
    fn phase_m_parity_tools_registered_with_right_shapes() {
        let tools = tools();
        // set_key_interp addresses a key by channel + key
        // index and names its mode.
        let t = tools
            .iter()
            .find(|t| t.name == "set_key_interp")
            .expect("set_key_interp registered");
        assert_eq!(t.schema["required"], json!(["channel", "key", "interp"]));
        assert_eq!(t.schema["properties"]["channel"]["type"], "integer");
        assert_eq!(t.schema["properties"]["key"]["type"], "integer");
        assert_eq!(t.schema["properties"]["interp"]["type"], "string");
        // particle_set_kind / particle_set_color address an
        // event by index; the colour travels as an integer.
        let t = tools
            .iter()
            .find(|t| t.name == "particle_set_kind")
            .expect("particle_set_kind registered");
        assert_eq!(t.schema["required"], json!(["index", "kind"]));
        assert_eq!(t.schema["properties"]["kind"]["type"], "string");
        let t = tools
            .iter()
            .find(|t| t.name == "particle_set_color")
            .expect("particle_set_color registered");
        assert_eq!(t.schema["required"], json!(["index", "color"]));
        assert_eq!(t.schema["properties"]["color"]["type"], "integer");
        // audio_set_muted needs the clip index and the flag.
        let t = tools
            .iter()
            .find(|t| t.name == "audio_set_muted")
            .expect("audio_set_muted registered");
        assert_eq!(t.schema["required"], json!(["index", "muted"]));
        assert_eq!(t.schema["properties"]["muted"]["type"], "boolean");
    }

    #[test]
    fn phase_m_parity_tools_run() {
        let tools = tools();
        let mut core = EditorCore::new();
        // set_key_interp: two graph_key calls build channel
        // 0 with two hermite keys; the second key goes step.
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 0.0, "value": 0.0
        }))
        .unwrap();
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 4.0, "value": 100.0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "set_key_interp", json!({
            "channel": 0, "key": 1, "interp": "step"
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["interp"], "step");
        assert!(matches!(
            core.project.channels[0].keys[1].interp,
            crate::graph::Interp::Step
        ));
        assert!(matches!(
            core.project.channels[0].keys[0].interp,
            crate::graph::Interp::Hermite
        ));
        // A key index past the channel's keys is the op's
        // own refusal, carried by the status line; a bad
        // mode name and a bad channel fail before the op.
        let err = call(&mut core, &tools, "set_key_interp", json!({
            "channel": 0, "key": 9, "interp": "step"
        }))
        .unwrap_err();
        assert_eq!(err, "No key 10 on graph channel 1 to set.");
        assert!(call(&mut core, &tools, "set_key_interp", json!({
            "channel": 0, "key": 0, "interp": "bouncy"
        }))
        .is_err());
        let err = call(&mut core, &tools, "set_key_interp", json!({
            "channel": 3, "key": 0, "interp": "step"
        }))
        .unwrap_err();
        assert_eq!(err, "no graph channel at index 3");

        // Particle tools: one event, kind cycled with a
        // mixed-case name, colour set as an integer.
        let idx = core.particle_add_event(0, 100.0, 100.0);
        assert_eq!(idx, 0);
        let r = call(&mut core, &tools, "particle_set_kind", json!({
            "index": 0, "kind": "Sparkle"
        }))
        .unwrap();
        assert_eq!(r["kind"], "Sparkle");
        assert!(matches!(
            core.project.particles[0].kind,
            crate::model::ParticleKind::Sparkle
        ));
        let r = call(&mut core, &tools, "particle_set_color", json!({
            "index": 0, "color": 0x35B6D9
        }))
        .unwrap();
        assert_eq!(r["color"], 0x35B6D9);
        assert_eq!(core.project.particles[0].color, 0x35B6D9);
        let err = call(&mut core, &tools, "particle_set_kind", json!({
            "index": 4, "kind": "burst"
        }))
        .unwrap_err();
        assert_eq!(err, "no particle event at index 4");

        // audio_set_muted: muting touches no file, but the
        // clip must still be placed with a real path —
        // add_audio_clip validates it (Wave 11). Mute, then
        // unmute.
        let a_wav = test_wav("m-a");
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": a_wav, "start_frame": 0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "audio_set_muted", json!({
            "index": 0, "muted": true
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["muted"], true);
        let r = call(&mut core, &tools, "audio_set_muted", json!({
            "index": 0, "muted": false
        }))
        .unwrap();
        assert_eq!(r["muted"], false);
        let err = call(&mut core, &tools, "audio_set_muted", json!({
            "index": 1, "muted": true
        }))
        .unwrap_err();
        assert_eq!(err, "no audio clip at index 1");
    }

    #[test]
    fn phase_j_mirror_figure_on_demo_core() {
        let tools = tools();
        let mut core = EditorCore::new();
        // The demo project has exactly one figure: index 0
        // mirrors across all its frames and reports through
        // the status line with the project summary.
        let r = call(&mut core, &tools, "mirror_figure", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["figure"], 0);
        assert!(r["status"].is_string());
        assert_eq!(r["project"]["figures"], 1);
        // Out-of-range and missing figures are clean errors
        // (the shared check_figure wording).
        let err = call(&mut core, &tools, "mirror_figure", json!({ "figure": 1 })).unwrap_err();
        assert_eq!(err, "figure index 1 out of range (project has 1 figure(s))");
        assert!(call(&mut core, &tools, "mirror_figure", json!({})).is_err());
    }

    #[test]
    fn phase_j_audio_normalise_all_on_demo_core() {
        let tools = tools();
        let mut core = EditorCore::new();
        // The demo project has no audio clips: the default
        // and boundary targets run cleanly and adjust
        // nothing.
        let r = call(&mut core, &tools, "audio_normalise_all", json!({})).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["adjusted"], 0);
        assert!(r["status"].is_string());
        let r = call(&mut core, &tools, "audio_normalise_all", json!({ "target_peak": 1.0 }))
            .unwrap();
        assert_eq!(r["adjusted"], 0);
        // The (0, 1] range is enforced before the op runs:
        // nought, negatives, anything above 1 and a
        // non-number are all clean errors.
        for bad in [0.0, -0.25, 1.5] {
            assert!(
                call(&mut core, &tools, "audio_normalise_all", json!({ "target_peak": bad }))
                    .is_err(),
                "target_peak {bad} must be refused"
            );
        }
        assert!(
            call(&mut core, &tools, "audio_normalise_all", json!({ "target_peak": "0.9" }))
                .is_err()
        );
        assert!(core.project.audio.is_empty());
    }

    #[test]
    fn phase_k_parity_tools_registered_with_right_shapes() {
        let tools = tools();
        // duplicate_graph_channel needs the channel index
        // and the figure to clone onto.
        let t = tools
            .iter()
            .find(|t| t.name == "duplicate_graph_channel")
            .expect("duplicate_graph_channel registered");
        assert_eq!(t.schema["required"], json!(["channel", "to_figure"]));
        assert_eq!(t.schema["properties"]["channel"]["type"], "integer");
        assert_eq!(t.schema["properties"]["to_figure"]["type"], "integer");
        // delete_graph_channel needs just the channel index.
        let t = tools
            .iter()
            .find(|t| t.name == "delete_graph_channel")
            .expect("delete_graph_channel registered");
        assert_eq!(t.schema["required"], json!(["channel"]));
        assert_eq!(t.schema["properties"]["channel"]["type"], "integer");
        // fx_set_kind needs the clip index and a kind name.
        let t = tools
            .iter()
            .find(|t| t.name == "fx_set_kind")
            .expect("fx_set_kind registered");
        assert_eq!(t.schema["required"], json!(["index", "kind"]));
        assert_eq!(t.schema["properties"]["index"]["type"], "integer");
        assert_eq!(t.schema["properties"]["kind"]["type"], "string");
    }

    #[test]
    fn phase_k_indexed_tools_validate_before_the_op() {
        let tools = tools();
        let mut core = EditorCore::new();
        // The demo project has no graph channels and no FX
        // clips: index 0 is out of range for all three
        // tools, with the pre-check wordings, and a missing
        // index is a clean error too.
        let err = call(
            &mut core,
            &tools,
            "duplicate_graph_channel",
            json!({ "channel": 0, "to_figure": 0 }),
        )
        .unwrap_err();
        assert_eq!(err, "no graph channel at index 0");
        assert!(
            call(&mut core, &tools, "duplicate_graph_channel", json!({ "to_figure": 0 }))
                .is_err()
        );
        let err = call(&mut core, &tools, "delete_graph_channel", json!({ "channel": 0 }))
            .unwrap_err();
        assert_eq!(err, "no graph channel at index 0");
        assert!(call(&mut core, &tools, "delete_graph_channel", json!({})).is_err());
        let err = call(
            &mut core,
            &tools,
            "fx_set_kind",
            json!({ "index": 0, "kind": "burst" }),
        )
        .unwrap_err();
        assert_eq!(err, "no FX clip at index 0");
        assert!(core.project.channels.is_empty());
        assert!(core.project.fx_clips.is_empty());
    }

    #[test]
    fn phase_k_duplicate_and_delete_graph_channel() {
        let tools = tools();
        let mut core = EditorCore::new();
        // Two figures, one RootX channel with two keys on
        // figure 0 — built through the handlers, the state
        // the panel would leave behind.
        call(&mut core, &tools, "add_figure", json!({ "kind": "humanoid" })).unwrap();
        assert_eq!(core.project.figures.len(), 2);
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 0.0, "value": 120.0
        }))
        .unwrap();
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 2.0, "value": 200.0
        }))
        .unwrap();
        assert_eq!(core.project.channels.len(), 1);

        // The duplicate lands as a second channel bound to
        // figure 1, keys carried over verbatim.
        let r = call(&mut core, &tools, "duplicate_graph_channel", json!({
            "channel": 0, "to_figure": 1
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["channel"], 0);
        assert!(r["status"].is_string());
        assert_eq!(r["project"]["figures"], 2);
        assert_eq!(core.project.channels.len(), 2);
        let clone = &core.project.channels[1];
        assert_eq!(clone.figure, 1);
        assert_eq!(clone.target, crate::graph::ChannelTarget::RootX);
        assert_eq!(clone.keys.len(), 2);
        assert_eq!(clone.keys[0].value, 120.0);
        assert_eq!(clone.keys[1].value, 200.0);

        // A second duplicate onto figure 1 refuses (the op:
        // figure 1 already has a RootX channel, so the
        // clone would be shadowed), as does duplicating
        // onto the channel's own figure; both surface as
        // errors and nothing is appended.
        assert!(call(&mut core, &tools, "duplicate_graph_channel", json!({
            "channel": 0, "to_figure": 1
        }))
        .is_err());
        assert!(call(&mut core, &tools, "duplicate_graph_channel", json!({
            "channel": 0, "to_figure": 0
        }))
        .is_err());
        assert_eq!(core.project.channels.len(), 2);

        // Delete removes the clone; the original stays.
        let r = call(&mut core, &tools, "delete_graph_channel", json!({ "channel": 1 }))
            .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["channel"], 1);
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(core.project.channels[0].figure, 0);
    }

    #[test]
    fn phase_k_fx_set_kind_changes_kind_and_rejects_bad_names() {
        let tools = tools();
        let mut core = EditorCore::new();
        // One burst clip on the demo figure.
        call(&mut core, &tools, "add_fx_clip", json!({
            "figure": 0, "node": 0, "kind": "burst", "start": 0.0, "duration": 5.0
        }))
        .unwrap();
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::Burst);

        let r = call(&mut core, &tools, "fx_set_kind", json!({
            "index": 0, "kind": "sparkle"
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["fx_index"], 0);
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::Sparkle);
        // The op's status names the kind in snake case.
        assert!(r["status"].as_str().unwrap().contains("sparkle"));

        // A kind name the parser does not know is a clean
        // error naming the bad value; the clip is untouched.
        let err = call(&mut core, &tools, "fx_set_kind", json!({
            "index": 0, "kind": "laser"
        }))
        .unwrap_err();
        assert!(err.contains("laser"), "error names the bad kind: {err}");
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::Sparkle);
        // A missing kind is a clean error too.
        assert!(call(&mut core, &tools, "fx_set_kind", json!({ "index": 0 })).is_err());
    }

    #[test]
    fn phase_l_settings_tools_registered_with_right_shapes() {
        let tools = tools();
        // set_project_fps needs just the rate.
        let t = tools
            .iter()
            .find(|t| t.name == "set_project_fps")
            .expect("set_project_fps registered");
        assert_eq!(t.schema["required"], json!(["fps"]));
        assert_eq!(t.schema["properties"]["fps"]["type"], "number");
        // set_stage_size needs both axes.
        let t = tools
            .iter()
            .find(|t| t.name == "set_stage_size")
            .expect("set_stage_size registered");
        assert_eq!(t.schema["required"], json!(["width", "height"]));
        assert_eq!(t.schema["properties"]["width"]["type"], "number");
        assert_eq!(t.schema["properties"]["height"]["type"], "number");
        // set_head_ring_scale needs just the multiplier.
        let t = tools
            .iter()
            .find(|t| t.name == "set_head_ring_scale")
            .expect("set_head_ring_scale registered");
        assert_eq!(t.schema["required"], json!(["scale"]));
        assert_eq!(t.schema["properties"]["scale"]["type"], "number");
    }

    #[test]
    fn phase_l_settings_tools_set_and_echo_status() {
        let tools = tools();
        let mut core = EditorCore::new();
        // Demo defaults: 12 fps, 960x720 stage, ring scale 1.
        assert_eq!(core.project.fps, 12.0);
        assert_eq!(core.project.stage, [960.0, 720.0]);
        assert_eq!(core.project.head_ring_scale, 1.0);

        // Frame rate: the field changes, the result echoes
        // the new value and the op's status, and the
        // summary follows.
        let r = call(&mut core, &tools, "set_project_fps", json!({ "fps": 24 })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["fps"], 24.0);
        assert_eq!(r["status"], "Frame rate set to 24 fps.");
        assert_eq!(r["project"]["fps"], 24.0);
        assert_eq!(core.project.fps, 24.0);
        // Out-of-range rates clamp inside the op.
        let r = call(&mut core, &tools, "set_project_fps", json!({ "fps": 500 })).unwrap();
        assert_eq!(r["fps"], 60.0);
        assert_eq!(r["status"], "Frame rate set to 60 fps.");
        assert_eq!(core.project.fps, 60.0);

        // Stage size: both axes land together.
        let r = call(&mut core, &tools, "set_stage_size", json!({
            "width": 800, "height": 600
        }))
        .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["stage"], json!([800.0, 600.0]));
        assert_eq!(r["status"], "Stage size set to 800×600.");
        assert_eq!(r["project"]["stage"], json!([800.0, 600.0]));
        assert_eq!(core.project.stage, [800.0, 600.0]);

        // Head-ring scale: the field changes and the
        // status names the multiplier.
        let r = call(&mut core, &tools, "set_head_ring_scale", json!({ "scale": 1.5 }))
            .unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["head_ring_scale"], 1.5);
        assert_eq!(r["status"], "Head ring size set to 1.5×.");
        assert_eq!(core.project.head_ring_scale, 1.5);
        // Out-of-range scales clamp inside the op.
        let r = call(&mut core, &tools, "set_head_ring_scale", json!({ "scale": 9 }))
            .unwrap();
        assert_eq!(r["head_ring_scale"], 2.0);
        assert_eq!(core.project.head_ring_scale, 2.0);
    }

    #[test]
    fn phase_f_parity_tools() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

        // --- set_project_easing (the editor's direct setter) ---
        let r = call(&mut core, &tools, "set_project_easing", json!({ "easing": "smooth" })).unwrap();
        assert_eq!(r["easing"], "smooth");
        assert_eq!(core.project.easing.label(), "smooth");
        for name in ["linear", "ease-in", "ease-out"] {
            let r = call(&mut core, &tools, "set_project_easing", json!({ "easing": name })).unwrap();
            assert_eq!(r["easing"], name);
            assert_eq!(core.project.easing.label(), name);
        }
        assert!(call(&mut core, &tools, "set_project_easing", json!({ "easing": "bouncy" })).is_err());
        assert!(call(&mut core, &tools, "set_project_easing", json!({})).is_err());

        // --- node style setters (selection-based editor ops) ---
        let r = call(&mut core, &tools, "set_node_cap", json!({
            "figure": 0, "node": 1, "cap": "square"
        }))
        .unwrap();
        assert_eq!(r["cap"], "square");
        assert_eq!(
            format!("{:?}", core.project.figures[0].def.nodes[1].style.cap),
            "Square"
        );
        assert!(call(&mut core, &tools, "set_node_cap", json!({
            "figure": 0, "node": 1, "cap": "flat"
        }))
        .is_err());
        assert!(call(&mut core, &tools, "set_node_cap", json!({
            "figure": 0, "node": 99, "cap": "round"
        }))
        .is_err());

        let r = call(&mut core, &tools, "set_node_color2", json!({
            "figure": 0, "node": 1, "color2": 0x112233
        }))
        .unwrap();
        assert_eq!(r["color2"], 0x112233);
        assert_eq!(core.project.figures[0].def.nodes[1].style.color2, Some(0x112233));
        // Omitting color2 clears the gradient, like fx_set_color2.
        let r = call(&mut core, &tools, "set_node_color2", json!({
            "figure": 0, "node": 1
        }))
        .unwrap();
        assert_eq!(r["color2"], Value::Null);
        assert_eq!(core.project.figures[0].def.nodes[1].style.color2, None);

        let r = call(&mut core, &tools, "set_node_opacity", json!({
            "figure": 0, "node": 1, "opacity": 0.5
        }))
        .unwrap();
        assert_eq!(r["opacity"], 0.5);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 0.5);
        // The op clamps into 0.05..=1.0 and the tool reads the
        // stored value back.
        let r = call(&mut core, &tools, "set_node_opacity", json!({
            "figure": 0, "node": 1, "opacity": 2.0
        }))
        .unwrap();
        assert_eq!(r["opacity"], 1.0);
        call(&mut core, &tools, "set_node_opacity", json!({
            "figure": 0, "node": 1, "opacity": 0.0
        }))
        .unwrap();
        let stored = core.project.figures[0].def.nodes[1].style.opacity;
        assert!((stored - 0.05).abs() < 1e-6, "clamped to the 0.05 floor, got {stored}");
        assert!(call(&mut core, &tools, "set_node_opacity", json!({
            "figure": 0, "node": 1, "opacity": -0.5
        }))
        .is_err());

        // --- graph key copy/paste round trip (graph_keys ops) ---
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 0.0, "value": 0.0
        }))
        .unwrap();
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "root_x", "frame": 4.0, "value": 100.0
        }))
        .unwrap();
        let r = call(&mut core, &tools, "graph_copy_keys", json!({
            "figure": 0, "target": "root_x"
        }))
        .unwrap();
        assert_eq!(r["key_count"], 2);
        let copied = r["keys"].clone();
        assert_eq!(copied[0]["frame"], 0.0);
        assert_eq!(copied[1]["frame"], 4.0);
        assert_eq!(copied[1]["value"], 100.0);
        assert_eq!(copied[0]["interp"], "hermite");
        // Copying a channel that does not exist is an error.
        assert!(call(&mut core, &tools, "graph_copy_keys", json!({
            "figure": 0, "target": "root_y"
        }))
        .is_err());

        // A destination channel holding one key at frame 1.
        call(&mut core, &tools, "graph_key", json!({
            "figure": 0, "target": "angle:2", "frame": 1.0, "value": 30.0
        }))
        .unwrap();
        // Paste the copied keys at frame 2: they rebase to
        // frames 2 and 6, and the frame-1 key (a full frame
        // away from the nearest pasted key) survives.
        let r = call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "angle:2", "keys": copied, "at_frame": 2.0
        }))
        .unwrap();
        assert_eq!(r["pasted"], 2);
        assert_eq!(r["channel_keys"], 3);
        let ang = core
            .project
            .channels
            .iter()
            .find(|c| matches!(c.target, crate::graph::ChannelTarget::Angle(2)))
            .unwrap();
        let frames: Vec<f32> = ang.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![1.0, 2.0, 6.0]);
        assert_eq!(ang.keys[2].value, 100.0);
        // Paste an explicit step key rebased onto frame 1: the
        // key already there is replaced, tangents default to 0.
        let r = call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "angle:2", "at_frame": 1.0,
            "keys": [{ "frame": 9.0, "value": 5.0, "interp": "step" }]
        }))
        .unwrap();
        assert_eq!(r["channel_keys"], 3, "replaced, not appended");
        let ang = core
            .project
            .channels
            .iter()
            .find(|c| matches!(c.target, crate::graph::ChannelTarget::Angle(2)))
            .unwrap();
        assert_eq!(ang.keys[0].frame, 1.0);
        assert_eq!(ang.keys[0].value, 5.0);
        assert!(matches!(ang.keys[0].interp, crate::graph::Interp::Step));
        // Paste failures: missing channel, empty keys, a key
        // without a value, a negative target frame.
        assert!(call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "root_y", "at_frame": 0.0,
            "keys": [{ "frame": 0.0, "value": 1.0 }]
        }))
        .is_err());
        assert!(call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "root_x", "keys": [], "at_frame": 0.0
        }))
        .is_err());
        assert!(call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "root_x", "at_frame": 0.0,
            "keys": [{ "frame": 0.0 }]
        }))
        .is_err());
        assert!(call(&mut core, &tools, "graph_paste_keys", json!({
            "figure": 0, "target": "root_x", "at_frame": -1.0,
            "keys": [{ "frame": 0.0, "value": 1.0 }]
        }))
        .is_err());
    }

    #[test]
    fn bad_arguments_error_instead_of_panicking() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

        let cases: Vec<(&str, Value)> = vec![
            ("add_figure", json!({ "kind": "dragon" })),
            ("add_figure", json!({})),
            ("get_pose", json!({ "figure": 99, "frame": 0 })),
            ("get_pose", json!({ "figure": 0 })),
            ("get_pose", json!({ "figure": -1, "frame": 0 })),
            ("set_pose", json!({ "figure": 0, "frame": 0 })),
            ("set_pose", json!({ "figure": 0, "frame": 0, "angles": [1.0, 2.0] })),
            ("set_pose", json!({ "figure": 0, "frame": 0, "angles": { "99": 1.0 } })),
            ("set_pose", json!({ "figure": 0, "frame": 0, "angles": "spin" })),
            ("set_pose", json!({ "figure": 0, "frame": 0, "root": [1.0] })),
            ("delete_frame", json!({ "frame": 0 })), // only frame
            ("set_frame_duration", json!({ "frame": 0, "duration": 0.0 })),
            ("set_camera", json!({ "frame": 0, "x": 0.0, "y": 0.0, "zoom": -1.0 })),
            ("add_fx_clip", json!({ "figure": 0, "node": 0, "kind": "laser", "start": 0.0, "duration": 5.0 })),
            ("add_fx_clip", json!({ "figure": 0, "node": 0, "kind": "burst", "start": 0.0, "duration": 0.0 })),
            ("fx_move", json!({ "index": 0, "start": 1.0 })), // no clips yet
            ("fx_move", json!({ "index": 0, "start": -2.0 })),
            ("fx_move", json!({ "start": 1.0 })), // missing index
            ("fx_resize", json!({ "index": 0, "duration": 5.0 })), // no clips yet
            ("fx_resize", json!({ "index": 0, "duration": 0.0 })),
            ("fx_duplicate", json!({ "index": 0 })), // no clips yet
            ("fx_delete", json!({ "index": 0 })), // no clips yet
            ("set_weather", json!({ "kind": "hail" })),
            ("add_audio_clip", json!({ "path": "", "start_frame": 0 })),
            ("graph_key", json!({ "figure": 0, "target": "spin", "frame": 0.0, "value": 1.0 })),
            ("graph_key", json!({ "figure": 0, "target": { "angle": 99 }, "frame": 0.0, "value": 1.0 })),
            ("layers", json!({ "figure": 7 })),
            // (thickness -2.0 is NOT a bad argument since
            // Wave 11: set_node clamps to [1, 18], panel-style.
            // A wrong-typed thickness still errors.)
            ("set_node", json!({ "figure": 0, "node": 0, "thickness": "thick" })),
            ("set_node", json!({ "figure": 0, "node": 0, "shape": "blob" })),
            ("add_node", json!({ "figure": 0, "parent": 42, "length": 10.0 })),
            ("delete_node", json!({ "figure": 0, "node": 0 })), // root refused
            ("new_project", json!({ "fps": -3.0 })),
            ("insert_inbetween", json!({})), // no following frame
            ("draw_stroke", json!({ "frame": 0, "points": [] })),
            ("draw_stroke", json!({ "frame": 0, "points": [[1.0]] })),
            ("draw_stroke", json!({ "frame": 9, "points": [[0.0, 0.0], [1.0, 1.0]] })),
            ("draw_stroke", json!({ "frame": 0, "points": [[0.0, 0.0]], "width": 0.0 })),
            ("smooth_strokes", json!({ "frame": 4 })),
            ("simplify_strokes", json!({ "frame": 99, "epsilon": 1.0 })),
            ("simplify_strokes", json!({ "epsilon": 0.0 })),
            ("mirror_channel_time", json!({ "figure": 0, "target": "root_x" })), // no channel yet
            ("scale_channel_values", json!({ "figure": 0, "target": "root_x", "factor": 2.0 })), // no channel yet
            ("erase_strokes", json!({ "frame": 0, "x": 0.0, "y": 0.0, "radius": -1.0 })),
            ("apply_nib", json!({ "frame": 0, "nib": "airbrush" })),
            ("fig3_select", json!({ "joint": 0 })), // no session yet
            ("fig3_rotate", json!({ "axis": 0, "degrees": 10.0 })), // no session yet
            ("fig3_orbit", json!({ "yaw": 1.0, "pitch": 1.0 })), // no session yet
            ("fig3_bake", json!({})), // no session yet
            ("fig3_mark_b", json!({})), // no session yet
            ("fig3_bake_animation", json!({ "frames": 4 })), // no session yet
            ("clip_create", json!({ "figure": 12 })),
            ("set_bend", json!({ "figure": 0, "frame": 0, "node": 99, "radians": 0.5 })),
            ("set_bend", json!({ "figure": 0, "frame": 0, "node": 1 })),
            ("figure_set_color", json!({ "figure": 0 })), // colour required
            ("figure_set_color", json!({ "figure": 9, "color": 1 })),
            ("figure_duplicate", json!({ "figure": 9 })),
            ("fx_set_color", json!({ "index": 0, "color": 1 })), // no clips yet
            ("fx_set_color2", json!({ "index": 0, "color2": 1 })), // no clips yet
            ("fx_set_anchor", json!({ "index": 0, "figure": 0, "node": 99 })),
            ("audio_fade", json!({})), // no audio clips
            ("audio_reverse", json!({})), // no audio clips
            ("audio_set_gain", json!({})), // gain required
            ("audio_set_gain", json!({ "gain": -0.5 })),
            ("audio_delete_clip", json!({ "index": 0 })), // no clips yet
            ("audio_move_clip", json!({ "start_frame": 1 })), // no clips yet
            ("delete_stroke", json!({ "index": 0 })), // no strokes yet
            ("set_project_easing", json!({})), // easing required
            ("set_project_easing", json!({ "easing": "bouncy" })),
            ("set_node_cap", json!({ "figure": 0, "node": 0 })), // cap required
            ("set_node_cap", json!({ "figure": 0, "node": 0, "cap": "flat" })),
            ("set_node_cap", json!({ "figure": 0, "node": 99, "cap": "round" })),
            ("set_node_color2", json!({ "figure": 0, "node": 99, "color2": 1 })),
            ("set_node_opacity", json!({ "figure": 0, "node": 0 })), // opacity required
            ("set_node_opacity", json!({ "figure": 0, "node": 0, "opacity": -0.5 })),
            ("graph_copy_keys", json!({ "figure": 0, "target": "root_x" })), // no channel yet
            ("graph_paste_keys", json!({ "figure": 0, "target": "root_x", "keys": [], "at_frame": 0.0 })),
            ("graph_paste_keys", json!({ "figure": 0, "target": "root_x", "at_frame": 0.0 })), // keys required
            ("duplicate_graph_channel", json!({ "channel": 0, "to_figure": 0 })), // no channels yet
            ("duplicate_graph_channel", json!({ "to_figure": 0 })), // channel required
            ("delete_graph_channel", json!({ "channel": 0 })), // no channels yet
            ("fx_set_kind", json!({ "index": 0, "kind": "burst" })), // no clips yet
            ("fx_set_kind", json!({ "index": 0, "kind": "laser" })), // unknown kind (and no clips yet)
            ("set_project_fps", json!({})), // fps required
            ("set_project_fps", json!({ "fps": "24" })), // fps must be a number
            ("set_stage_size", json!({ "width": 800 })), // height required
            ("set_stage_size", json!({ "width": 800, "height": "600" })), // height must be a number
            ("set_head_ring_scale", json!({})), // scale required
            ("set_head_ring_scale", json!({ "scale": "big" })), // scale must be a number
            ("set_key_interp", json!({ "channel": 0, "key": 0, "interp": "hermite" })), // no channels yet
            ("set_key_interp", json!({ "key": 0, "interp": "hermite" })), // channel required
            ("set_key_interp", json!({ "channel": 0, "key": 0 })), // interp required
            ("set_key_interp", json!({ "channel": 0, "key": 0, "interp": "bouncy" })), // unknown interp (and no channels yet)
            ("particle_set_kind", json!({ "index": 0, "kind": "burst" })), // no events yet
            ("particle_set_kind", json!({ "index": 0 })), // kind required
            ("particle_set_kind", json!({ "index": 0, "kind": "laser" })), // unknown kind (and no events yet)
            ("particle_set_color", json!({ "index": 0, "color": 16711680 })), // no events yet
            ("particle_set_color", json!({ "index": 0 })), // colour required
            ("particle_set_color", json!({ "index": 0, "color": "red" })), // colour must be an integer
            ("audio_set_muted", json!({ "index": 0, "muted": true })), // no clips yet
            ("audio_set_muted", json!({ "index": 0 })), // muted required
            ("audio_set_muted", json!({ "muted": true })), // index required
            ("audio_set_muted", json!({ "index": 0, "muted": "yes" })), // muted must be a boolean
        ];
        for (name, args) in cases {
            let r = call(&mut core, &tools, name, args.clone());
            assert!(r.is_err(), "{name} with {args} should error, got {r:?}");
        }
        // The project survived all of it untouched.
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(core.project.frames.len(), 1);
    }

    #[test]
    fn wave_x_state_tools_key_assign_and_unlink() {
        let tools = tools();
        let mut core = EditorCore::new();
        // Two clips from figure 0; both start life named
        // "Styk clip", so rename the second before using
        // name-accepting calls (first match would win).
        call(&mut core, &tools, "clip_create", json!({ "figure": 0 })).unwrap();
        call(&mut core, &tools, "clip_create", json!({ "figure": 0 })).unwrap();
        assert_eq!(core.project.clips.len(), 2);
        assert_eq!(core.project.figures[0].clip, Some(1));
        call(
            &mut core,
            &tools,
            "clip_rename",
            json!({ "index": 1, "name": "Wave" }),
        )
        .unwrap();

        // Compatibility reads by index and by name.
        let r = call(&mut core, &tools, "clip_compatible", json!({ "figure": 0, "clip": 0 }))
            .unwrap();
        assert_eq!(r["compatible"], true);
        let r = call(
            &mut core,
            &tools,
            "clip_compatible",
            json!({ "figure": 0, "clip_name": "Wave" }),
        )
        .unwrap();
        assert_eq!(r["clip"], 1);
        assert_eq!(r["clip_name"], "Wave");
        // A name that matches nothing is an error naming it.
        let e = call(
            &mut core,
            &tools,
            "clip_compatible",
            json!({ "figure": 0, "clip_name": "Nope" }),
        )
        .unwrap_err();
        assert_eq!(e, "no clip named 'Nope'");

        // Assign by name; re-assigning is the reported no-op.
        let r = call(
            &mut core,
            &tools,
            "figure_assign_clip",
            json!({ "figure": 0, "clip_name": "Styk clip" }),
        )
        .unwrap();
        assert_eq!(r["clip"], 0);
        assert_eq!(core.project.figures[0].clip, Some(0));
        let r = call(
            &mut core,
            &tools,
            "figure_assign_clip",
            json!({ "figure": 0, "clip": 0 }),
        )
        .unwrap();
        assert_eq!(r["already_linked"], true);

        // Key a state at frame 2 by name, with offset + speed:
        // exactly one undo step, key stored verbatim.
        let depth = core.undo_depth();
        let r = call(
            &mut core,
            &tools,
            "figure_set_state",
            json!({
                "figure": 0,
                "frame": 2,
                "clip_name": "Wave",
                "offset": 1,
                "speed": 2.0
            }),
        )
        .unwrap();
        assert_eq!(core.undo_depth(), depth + 1, "one undo step per state write");
        assert_eq!(r["clip"], 1);
        assert_eq!(r["offset"], 1);
        let key = core.project.figures[0].states[0];
        assert_eq!((key.frame, key.clip, key.offset, key.speed), (2, 1, 1, 2.0));

        // The list read resolves names for base + keys, and
        // clip_list now counts the state-keyed usage.
        let r = call(&mut core, &tools, "figure_list_states", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["base_clip"], 0);
        assert_eq!(r["base_clip_name"], "Styk clip");
        assert_eq!(r["count"], 1);
        assert_eq!(r["states"][0]["clip_name"], "Wave");
        let r = call(&mut core, &tools, "clip_list", json!({})).unwrap();
        assert_eq!(r["clips"][1]["used_by"], json!([0]), "state usage counts");

        // Replacing the key resets omitted offset/speed.
        call(
            &mut core,
            &tools,
            "figure_set_state",
            json!({ "figure": 0, "frame": 2, "clip": 1 }),
        )
        .unwrap();
        let key = core.project.figures[0].states[0];
        assert_eq!((key.offset, key.speed), (0, 1.0));
        assert_eq!(core.project.figures[0].states.len(), 1);

        // Undo restores the pre-replace key (one step back).
        core.undo();
        let key = core.project.figures[0].states[0];
        assert_eq!((key.offset, key.speed), (1, 2.0));

        // Clear: one step; a second clear refuses in the op's
        // voice; a state key past the timeline end is refused
        // by check_frame at authoring time.
        call(
            &mut core,
            &tools,
            "figure_clear_state",
            json!({ "figure": 0, "frame": 2 }),
        )
        .unwrap();
        assert!(core.project.figures[0].states.is_empty());
        let e = call(
            &mut core,
            &tools,
            "figure_clear_state",
            json!({ "figure": 0, "frame": 2 }),
        )
        .unwrap_err();
        assert!(e.contains("has no state key at frame 3"), "op voice: {e}");
        assert!(call(
            &mut core,
            &tools,
            "figure_set_state",
            json!({ "figure": 0, "frame": 99, "clip": 0 }),
        )
        .is_err());

        // Unlink drops the base link; the second unlink is
        // the reported no-op.
        let r = call(&mut core, &tools, "figure_unlink_clip", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(core.project.figures[0].clip, None);
        let r = call(&mut core, &tools, "figure_unlink_clip", json!({ "figure": 0 })).unwrap();
        assert_eq!(r["already_unlinked"], true);

        // Ranged clip creation: [1, 3) bakes two frames and
        // links the figure.
        let r = call(
            &mut core,
            &tools,
            "clip_create_range",
            json!({ "figure": 0, "start": 1, "end": 3 }),
        )
        .unwrap();
        assert_eq!(r["frames"], 2);
        assert_eq!(r["loops_on_figure"], true);
        assert_eq!(core.project.clips.len(), 3);
        // An empty range refuses through the op's status.
        assert!(call(
            &mut core,
            &tools,
            "clip_create_range",
            json!({ "figure": 0, "start": 2, "end": 2 }),
        )
        .is_err());
    }
}
