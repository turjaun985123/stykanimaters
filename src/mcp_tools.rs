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
use crate::model::{AudioClip, CameraKey, FigureDef, Frame, Pose, Project, Shape};
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
        other => Err(format!(
            "unknown fx kind '{other}' (expected burst, sparkle, smoke, beam, aura, trail, confetti or shock_ring)"
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
    let length = opt_f32(args, "length")?.unwrap_or(130.0);
    // Same construction as the editor's own add_fx_clip op.
    let seed = 11 + core.project.fx_clips.len() as u32 * 7;
    core.project.fx_clips.push(crate::model::FxClipEntry {
        clip: ParticleClip {
            start,
            duration,
            kind,
            color,
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
        other => {
            return Err(format!(
                "unknown weather kind '{other}' (expected \"rain\", \"snow\", \"fireflies\" or \"off\")"
            ))
        }
    };
    let amount = opt_f32(args, "amount")?.unwrap_or(90.0).clamp(0.0, 200.0);
    let wind = opt_f32(args, "wind")?.unwrap_or(20.0);
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
    core.project.audio.push(AudioClip { path: path.to_string(), start_frame });
    Ok(json!({
        "ok": true,
        "audio_index": core.project.audio.len() - 1,
        "path": path,
        "start_frame": start_frame,
    }))
}

fn tool_graph_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = req_usize(args, "figure")?;
    check_figure(core, fi)?;
    let target_v = get(args, "target").ok_or("missing required argument 'target'")?;
    let target = if let Some(s) = target_v.as_str() {
        match s {
            "root_x" => crate::graph::ChannelTarget::RootX,
            "root_y" => crate::graph::ChannelTarget::RootY,
            other => {
                return Err(format!(
                    "unknown graph target '{other}' (expected \"root_x\", \"root_y\" or {{\"angle\": node}})"
                ))
            }
        }
    } else if let Some(map) = target_v.as_object() {
        match map.get("angle") {
            Some(v) => {
                let ni = as_usize(v, "target.angle")?;
                check_node(core, fi, ni)?;
                crate::graph::ChannelTarget::Angle(ni)
            }
            None => {
                return Err("argument 'target' object must contain an 'angle' node index".into())
            }
        }
    } else {
        return Err("argument 'target' must be \"root_x\", \"root_y\" or {\"angle\": node}".into());
    };
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

// ---------------------------------------------------------------------
// The tool table
// ---------------------------------------------------------------------

/// All content-building tools, in the order agents usually need them.
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
            description: "Add a timed particle FX clip anchored to a figure's node. start/duration are in frames.",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "node": { "type": "integer" },
                    "kind": { "type": "string", "enum": ["burst", "sparkle", "smoke", "beam", "aura", "trail", "confetti", "shock_ring"] },
                    "start": { "type": "number" },
                    "duration": { "type": "number" },
                    "color": { "type": "integer", "description": "0xRRGGBB (default: figure colour)" },
                    "length": { "type": "number", "description": "Beam reach / trail span (default 130)" }
                }),
                &["figure", "node", "kind", "start", "duration"],
            ),
            handler: tool_add_fx_clip,
        },
        Tool {
            name: "set_weather",
            description: "Set continuous weather over the whole timeline, or turn it off.",
            schema: obj_schema(
                json!({
                    "kind": { "type": "string", "enum": ["rain", "snow", "fireflies", "off"] },
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
            name: "graph_key",
            description: "Add a graph-editor channel key: target is \"root_x\", \"root_y\" or {\"angle\": node}; frame and value are numbers (degrees for angles).",
            schema: obj_schema(
                json!({
                    "figure": { "type": "integer" },
                    "target": { "description": "\"root_x\", \"root_y\" or {\"angle\": nodeIndex}" },
                    "frame": { "type": "number" },
                    "value": { "type": "number" }
                }),
                &["figure", "target", "frame", "value"],
            ),
            handler: tool_graph_key,
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
        assert_eq!(tools.len(), 20);
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 20, "tool names must be unique");
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
