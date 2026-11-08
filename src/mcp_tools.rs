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

fn req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("argument '{key}' must be a number")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| Some(f as f32))
            .ok_or_else(|| format!("argument '{key}' must be a number")),
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
    let x = arr[0]
        .as_f64()
        .ok_or_else(|| format!("argument '{key}[0]' must be a number"))? as f32;
    let y = arr[1]
        .as_f64()
        .ok_or_else(|| format!("argument '{key}[1]' must be a number"))? as f32;
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
    };
    core.current = 0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
    core.playing = false;
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
    if core.project.frames.is_empty() {
        return Err("project has no frames to copy".into());
    }
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
    core.project.frames[frame].duration = duration;
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
    for (i, d) in durations.into_iter().take(updated).enumerate() {
        core.project.frames[i].duration = d;
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
                let deg = v
                    .as_f64()
                    .ok_or_else(|| format!("argument 'angles[{i}]' must be a number"))?
                    as f32;
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
                let deg = v
                    .as_f64()
                    .ok_or_else(|| format!("argument 'angles.{k}' must be a number"))?
                    as f32;
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

fn tool_set_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    let ni = req_usize(args, "node")?;
    check_node(core, fi, ni)?;
    let length = opt_f32(args, "length")?;
    if let Some(l) = length {
        if l < 0.0 {
            return Err("argument 'length' must not be negative".into());
        }
    }
    let thickness = opt_f32(args, "thickness")?;
    if let Some(t) = thickness {
        if t <= 0.0 {
            return Err("argument 'thickness' must be positive".into());
        }
    }
    let color = opt_color(args, "color")?;
    let shape = match get(args, "shape") {
        Some(v) => Some(parse_shape(v)?),
        None => None,
    };
    let node = &mut core.project.figures[fi].def.nodes[ni];
    if let Some(l) = length {
        node.length = l;
    }
    if let Some(t) = thickness {
        node.thickness = t;
    }
    if let Some(c) = color {
        node.color = Some(c);
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
    core.project.set_camera_key(frame, x, y, zoom);
    if let Some(a) = angle {
        if let Some(k) = core.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = a;
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
    let duration = req_f32(args, "duration")?;
    if duration <= 0.0 {
        return Err("argument 'duration' must be positive".into());
    }
    let color = opt_color(args, "color")?.unwrap_or(core.project.figures[fi].color);
    let color2 = opt_color(args, "color2")?;
    let length = opt_f32(args, "length")?.unwrap_or(130.0);
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
                "kind": info.kind,
                "start": info.start,
                "duration": info.duration,
                "end": info.end,
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
    // The phase-E EditorCore op moves by a whole-frame delta
    // (snapshotting, so MCP moves are undoable like the
    // panel's); the tool's contract stays the absolute start,
    // so the delta comes from the clip's current start and
    // the result is read back.
    let current = core.project.fx_clips[idx].clip.start;
    let delta = (start - current).round() as i32;
    core.fx_move_clip(idx, delta);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "start": core.project.fx_clips[idx].clip.start,
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
    crate::fx_edit::save_clip_preset(&clip, name).map_err(|e| e.to_string())?;
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "preset": name,
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
    core.project.fx_clips.push(crate::model::FxClipEntry { clip, figure, node: 0 });
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
        core.project.weather = None;
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
    // Storm is heavier by default: more particles, stronger drift.
    let (default_amount, default_wind) =
        if wk == WeatherKind::Storm { (140.0, 30.0) } else { (90.0, 20.0) };
    let amount = opt_f32(args, "amount")?.unwrap_or(default_amount).clamp(0.0, 200.0);
    let wind = opt_f32(args, "wind")?.unwrap_or(default_wind);
    core.project.weather = Some(Weather { kind: wk, amount, wind });
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
    core.project.audio.push(AudioClip { path: path.to_string(), start_frame, gain: 1.0 });
    Ok(json!({
        "ok": true,
        "audio_index": core.project.audio.len() - 1,
        "path": path,
        "start_frame": start_frame,
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

fn tool_audio_fade(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let idx = check_last_audio(core)?;
    core.audio_fade_last();
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_reverse(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let idx = check_last_audio(core)?;
    core.audio_reverse_last();
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
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
        "gain": gain,
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
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_lowpass(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_lowpass_at(idx);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_trim_silence(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_trim_silence_at(idx);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_pitch(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_pitch_at(idx);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_compress(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    core.audio_compress_at(idx);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "status": core.status,
        "project": project_summary(core),
    }))
}

fn tool_audio_split_silences(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = check_audio_index(core, req_usize(args, "index")?)?;
    let added = core.audio_split_silences_at(idx);
    Ok(json!({
        "ok": true,
        "audio_index": idx,
        "added": added,
        "count": core.project.audio.len(),
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

/// Parse the graph `target` argument shared by graph_key and the
/// channel transform tools: "root_x"/"root_y" (or "rootx"/"rooty"),
/// "angle:N" / "length:N" strings, or an object {"angle": node} /
/// {"length": node}. Node indices are range-checked against the
/// figure.
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
        for (prefix, is_angle) in [("angle:", true), ("length:", false)] {
            if let Some(rest) = s.strip_prefix(prefix) {
                let ni: usize = rest.parse().map_err(|_| {
                    format!("unknown graph target '{s}' (the {prefix} index must be an integer)")
                })?;
                check_node(core, fi, ni)?;
                return Ok(if is_angle {
                    ChannelTarget::Angle(ni)
                } else {
                    ChannelTarget::Length(ni)
                });
            }
        }
        return Err(format!(
            "unknown graph target '{s}' (expected \"root_x\", \"root_y\", \"angle:N\", \"length:N\" or an object with an angle/length key)"
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
        return Err("argument 'target' object must contain an 'angle' or 'length' node index".into());
    }
    Err("argument 'target' must be \"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": node}".into())
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
    let ch = crate::graph::channel_mut(&mut core.project.channels, fi, target);
    ch.add_key(frame, value);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "value": value,
        "channel_keys": ch.keys.len(),
        "channels": core.project.channels.len(),
    }))
}

fn tool_layers(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    if let Some(locked) = opt_bool(args, "locked")? {
        core.project.figures[fi].locked = locked;
    }
    if let Some(hidden) = opt_bool(args, "hidden")? {
        core.project.figures[fi].hidden = hidden;
    }
    if let Some(opacity) = opt_f32(args, "opacity")? {
        // layers::set_opacity clamps to the supported 0.05..=1.0 range.
        crate::layers::set_opacity(&mut core.project, fi, opacity);
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
    core.cycle_clip_loop(index);
    Ok(json!({ "ok": true, "status": core.status, "project": project_summary(core) }))
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
                Some(x) => x
                    .as_f64()
                    .map(|f| Some(f as f32))
                    .ok_or_else(|| format!("argument 'keys[{i}].{name}' must be a number")),
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
        let x = pair[0]
            .as_f64()
            .ok_or_else(|| format!("argument '{key}[{i}][0]' must be a number"))? as f32;
        let y = pair[1]
            .as_f64()
            .ok_or_else(|| format!("argument '{key}[{i}][1]' must be a number"))? as f32;
        points.push([x, y]);
    }
    Ok(points)
}

fn tool_draw_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = req_usize(args, "frame")?;
    check_frame(core, frame)?;
    let points = req_points(args, "points")?;
    if points.is_empty() {
        return Err("argument 'points' must contain at least one point".into());
    }
    // Defaults mirror the editor's own pen: light grey, width 4.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or(4.0);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    let n_points = points.len();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
    });
    Ok(json!({
        "ok": true,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_draw_shape(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Geometry comes from paint::shape_points so MCP shapes match
    // the editor's own shape tool point-for-point. The stroke goes
    // on the CURRENT frame, like a stroke drawn by hand.
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
    // Same pen defaults as draw_stroke (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    let points = crate::paint::shape_points(kind, [x1, y1], [x2, y2]);
    let n_points = points.len();
    let frame = core.current;
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
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
    // straight up (rotation -90 degrees).
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
    // Same pen defaults as draw_shape (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    let points = crate::paint::regular_polygon_points([cx, cy], radius, sides, -90.0);
    let n_points = points.len();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
    });
    Ok(json!({
        "ok": true,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": crate::draw_ops::stroke_count(&core.project, frame),
    }))
}

fn tool_draw_star(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // Geometry comes from paint::star_points: a star outline on
    // the CURRENT frame, like draw_shape — outer radius 'radius',
    // inner radius 45% of it, first point straight up.
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
    // Same pen defaults as draw_shape (light grey; the active
    // brush's width), with the same optional overrides.
    let color = opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    let points = crate::paint::star_points([cx, cy], radius, radius * 0.45, points_count, -90.0);
    let n_points = points.len();
    let frame = core.current;
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
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
                "camera": { "yaw": st.camera.yaw, "pitch": st.camera.pitch },
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
                "used_by": core
                    .project
                    .figures
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.clip == Some(i))
                    .map(|(fi, _)| fi)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let count = clips.len();
    Ok(json!({ "clips": clips, "count": count }))
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
    let radians = req_f32(args, "radians")?;
    let fr = &mut core.project.frames[frame];
    while fr.bends.len() <= fi {
        fr.bends.push(Vec::new());
    }
    let row = &mut fr.bends[fi];
    while row.len() <= ni {
        row.push(0.0);
    }
    row[ni] = radians;
    Ok(json!({
        "ok": true,
        "figure": fi,
        "frame": frame,
        "node": ni,
        "radians": radians,
    }))
}

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
            description: "Set a figure's colour (0xRRGGBB) — the colour its segments draw in unless a segment carries its own colour override.",
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
            description: "Append a copy of the current frame after it and move to it.",
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
            description: "Set a frame's hold-time multiplier (1.0 = one beat at the project fps).",
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
            description: "Set hold-time multipliers for frames 0..n in one call (same positive-value rule as set_frame_duration; extra entries beyond the frame count are ignored).",
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
            description: "Read a figure's pose in a frame: {root: [x, y], angles: [degrees per node]}.",
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
            description: "Write a figure's pose in a frame. 'angles' is either a full array (one angle per node, degrees) or an object mapping node index to degrees; 'root' is [x, y].",
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
            description: "Edit a figure definition's segment: length, thickness, colour override, or shape (\"line\" | \"circle\" | \"polygon\" | side count 3-12).",
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
            description: "Set a segment's opacity (the style panel's opacity stepper; clamped to 0.05-1.0 so a segment can never be made fully invisible). One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "opacity": { "type": "number", "description": "Opacity, clamped to 0.05-1.0" }
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
            description: "Upsert a camera key at a frame: centre (x, y), zoom, optional rotation angle in degrees.",
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
            description: "Add a timed particle FX clip anchored to a figure's node. start/duration are in frames. 'color2' optionally sets a gradient end colour (particles blend colour -> color2).",
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
            description: "List all timed FX clips: index, figure, node, kind, start, duration and end frame.",
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
            description: "Set an FX clip's duration in frames. One undo step.",
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
            description: "Duplicate a particle event (one of the project's timed particle bursts, listed in the stats' particle count): the copy is appended to the event list and its new index is returned. One undo step.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_particle_duplicate_event,
        },
        Tool {
            name: "set_weather",
            description: "Set continuous weather over the whole timeline, or turn it off.",
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
            description: "Place an audio file on the timeline starting at a frame.",
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
            description: "Apply a fade-in/fade-out to the LAST audio clip: a processed copy of its file is written and the clip repointed (the editor's Fade op). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_audio_fade,
        },
        Tool {
            name: "audio_reverse",
            description: "Reverse the LAST audio clip: a processed copy of its file is written and the clip repointed (the editor's Reverse op). One undo step.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_audio_reverse,
        },
        Tool {
            name: "audio_set_gain",
            description: "Apply a gain to the LAST audio clip (the editor's gain op; 1.0 is unity, 0.5 halves the level). One undo step.",
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
            description: "Add a graph-editor channel key: target is \"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": node}; frame and value are numbers (degrees for angles).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": nodeIndex}" },
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
            description: "Apply an easing preset to the tangents of every graph channel key in one pass: \"smooth\", \"ease-in\", \"ease-out\", \"ease-inout\" or \"linear\". One undo step.",
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
            description: "Mirror one graph channel's keys in time around 'centre' (a frame coordinate; default: the timeline's middle frame), so that channel's animation plays backwards. Target addressing matches graph_key (\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": node}). Errors when the channel does not exist. One undo step.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": nodeIndex}" },
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
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": nodeIndex}" },
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
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": nodeIndex}" }
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
                    "target": { "description": "\"root_x\", \"root_y\", \"angle:N\", \"length:N\" or {\"angle\": nodeIndex}" },
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
            name: "layers",
            description: "Apply layer state to a figure: locked, hidden, opacity (0.05-1.0), and solo (true solos this figure in the editor, false clears its solo).",
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
            description: "Append a freehand stroke (polyline) to a frame's drawing layer. points is an array of [x, y] stage coordinates; color is 0xRRGGBB (default light grey), width in stage units (default 4, the editor's pen).",
            schema: obj_schema(
                json!({
                    "frame": { "type": "integer" },
                    "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 } },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" }
                }),
                &["frame", "points"],
            ),
            handler: tool_draw_stroke,
        },
        Tool {
            name: "draw_shape",
            description: "Append a geometric shape as a stroke on the CURRENT frame: kind is \"line\" (2 points), \"rect\" (closed rectangle, 5 points) or \"ellipse\" (25 points), from corner (x1, y1) to corner (x2, y2) in stage coordinates. color is 0xRRGGBB (default light grey), width defaults to the active brush width.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "enum": ["line", "rect", "ellipse"] },
                    "x1": { "type": "number" },
                    "y1": { "type": "number" },
                    "x2": { "type": "number" },
                    "y2": { "type": "number" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" }
                }),
                &["kind", "x1", "y1", "x2", "y2"],
            ),
            handler: tool_draw_shape,
        },
        Tool {
            name: "draw_polygon",
            description: "Append a closed regular polygon as a stroke: centred at (cx, cy) with the given radius, 'sides' vertices (default 5, at least 3) plus the closing point, first vertex straight up. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width.",
            schema: obj_schema(
                json!({
                    "cx": { "type": "number" },
                    "cy": { "type": "number" },
                    "radius": { "type": "number" },
                    "sides": { "type": "integer", "minimum": 3, "description": "Vertex count (default 5)" },
                    "frame": { "type": "integer", "description": "Target frame (default: current frame)" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" }
                }),
                &["cx", "cy", "radius"],
            ),
            handler: tool_draw_polygon,
        },
        Tool {
            name: "draw_star",
            description: "Append a star outline as a stroke on the CURRENT frame: centred at (cx, cy), outer radius 'radius', inner radius 45% of it, 'points' spikes (default 5), first spike straight up. color is 0xRRGGBB (default light grey), width defaults to the active brush width.",
            schema: obj_schema(
                json!({
                    "cx": { "type": "number" },
                    "cy": { "type": "number" },
                    "radius": { "type": "number" },
                    "points": { "type": "integer", "minimum": 2, "description": "Spike count (default 5)" },
                    "color": { "type": "integer", "description": "0xRRGGBB" },
                    "width": { "type": "number" }
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
            description: "List the project's movieclip definitions: index, name, frame count, node count, and which figures loop each clip.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_clip_list,
        },
        Tool {
            name: "set_clip_loop",
            description: "Cycle a movieclip's loop mode one step: Loop -> PingPong -> Once -> Loop. The returned status names the new mode; an out-of-range index is a no-op reported in the status.",
            schema: obj_schema(
                json!({ "index": { "type": "integer" } }),
                &["index"],
            ),
            handler: tool_set_clip_loop,
        },
        Tool {
            name: "set_bend",
            description: "Set one segment's bend (radians) for a figure on a frame. Positive/negative bend arcs the segment to opposite sides; 0 is straight. Frame.bends rows/cols are extended with zeros as needed.",
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

    #[test]
    fn tool_table_is_complete_and_unique() {
        let tools = tools();
        assert_eq!(tools.len(), 84);
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 84, "tool names must be unique");
        for t in &tools {
            assert_eq!(t.schema["type"], "object", "tool {} schema", t.name);
        }
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
        call(&mut core, &tools, "add_audio_clip", json!({ "path": "music.wav", "start_frame": 0 })).unwrap();
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
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": "a.wav", "start_frame": 0
        }))
        .unwrap();
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": "b.wav", "start_frame": 2
        }))
        .unwrap();
        let r = call(&mut core, &tools, "audio_move_clip", json!({ "start_frame": 7 })).unwrap();
        assert_eq!(r["start_frame"], 7);
        assert_eq!(core.project.audio[1].start_frame, 7, "the LAST clip moved");
        assert_eq!(core.project.audio[0].start_frame, 0, "the first clip stayed");
        let r = call(&mut core, &tools, "audio_delete_clip", json!({ "index": 0 })).unwrap();
        assert_eq!(r["count"], 1);
        assert_eq!(core.project.audio[0].path, "b.wav");
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
        call(&mut core, &tools, "add_audio_clip", json!({
            "path": "a.wav", "start_frame": 0
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
            ("set_node", json!({ "figure": 0, "node": 0, "thickness": -2.0 })),
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
        ];
        for (name, args) in cases {
            let r = call(&mut core, &tools, name, args.clone());
            assert!(r.is_err(), "{name} with {args} should error, got {r:?}");
        }
        // The project survived all of it untouched.
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(core.project.frames.len(), 1);
    }
}
