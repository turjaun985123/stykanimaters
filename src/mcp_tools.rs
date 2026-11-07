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
        other => Err(format!(
            "unknown fx kind '{other}' (expected burst, sparkle, smoke, beam, aura, trail, confetti, shock_ring, ember, fountain, orbit, lightning, bubbles, leaves, meteor, petals, splash or spiral)"
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
    if !crate::fx_edit::move_clip(&mut core.project, idx, start) {
        return Err(format!("no fx clip at index {idx}"));
    }
    Ok(json!({ "ok": true, "fx_index": idx, "start": start }))
}

fn tool_fx_resize(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let duration = req_f32(args, "duration")?;
    if duration <= 0.0 {
        return Err("argument 'duration' must be positive".into());
    }
    if !crate::fx_edit::resize_clip(&mut core.project, idx, duration) {
        return Err(format!("no fx clip at index {idx}"));
    }
    Ok(json!({ "ok": true, "fx_index": idx, "duration": duration }))
}

fn tool_fx_duplicate(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = req_usize(args, "index")?;
    let Some(new_idx) = crate::fx_edit::duplicate_clip(&mut core.project, idx) else {
        return Err(format!("no fx clip at index {idx}"));
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
    if !crate::fx_edit::delete_clip(&mut core.project, idx) {
        return Err(format!("no fx clip at index {idx}"));
    }
    Ok(json!({
        "ok": true,
        "deleted": idx,
        "count": core.project.fx_clips.len(),
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
                    "kind": { "type": "string", "enum": ["burst", "sparkle", "smoke", "beam", "aura", "trail", "confetti", "shock_ring", "ember", "fountain", "orbit", "lightning", "bubbles", "leaves", "meteor", "petals", "splash", "spiral"] },
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
            name: "fx_list",
            description: "List all timed FX clips: index, figure, node, kind, start, duration and end frame.",
            schema: obj_schema(json!({}), &[]),
            handler: tool_fx_list,
        },
        Tool {
            name: "fx_move",
            description: "Move an FX clip to a new start frame (clamped at 0).",
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
            description: "Set an FX clip's duration in frames.",
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
            description: "Duplicate an FX clip: the copy is inserted right after it and starts where the original ends.",
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
            description: "Delete an FX clip by index.",
            schema: obj_schema(
                json!({
                    "index": { "type": "integer" }
                }),
                &["index"],
            ),
            handler: tool_fx_delete,
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
        assert_eq!(tools.len(), 34);
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 34, "tool names must be unique");
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
    #[test]
    fn wave9_stats_and_bulk_durations() {
        let tools = tools();
        let mut core = EditorCore::new();
        call(&mut core, &tools, "new_project", json!({})).unwrap();

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
        // including the wave-7 kinds (lightning, bubbles, leaves)
        // and ember/fountain/orbit, which predate the fx timeline.
        for (name, debug) in [
            ("ember", "Ember"),
            ("fountain", "Fountain"),
            ("orbit", "Orbit"),
            ("lightning", "Lightning"),
            ("bubbles", "Bubbles"),
            ("leaves", "Leaves"),
        ] {
            let r = call(&mut core, &tools, "add_fx_clip", json!({
                "figure": 0, "node": 0, "kind": name, "start": 0.0, "duration": 6.0
            }))
            .unwrap();
            assert_eq!(r["kind"], debug, "{name}");
        }
        assert_eq!(core.project.fx_clips.len(), 6);

        // fx_list reports the fx_edit view of the same clips.
        let r = call(&mut core, &tools, "fx_list", json!({})).unwrap();
        assert_eq!(r["count"], 6);
        assert_eq!(r["clips"][3]["kind"], "lightning");
        assert_eq!(r["clips"][3]["figure"], 0);
        assert_eq!(r["clips"][3]["node"], 0);
        assert_eq!(r["clips"][3]["start"], 0.0);
        assert_eq!(r["clips"][3]["end"], 6.0);

        // Move + resize go through fx_edit's ops.
        let r = call(&mut core, &tools, "fx_move", json!({ "index": 3, "start": 10.0 })).unwrap();
        assert_eq!(r["start"], 10.0);
        assert_eq!(core.project.fx_clips[3].clip.start, 10.0);
        call(&mut core, &tools, "fx_resize", json!({ "index": 3, "duration": 4.0 })).unwrap();
        assert_eq!(core.project.fx_clips[3].clip.duration, 4.0);

        // Duplicate lands right after the original, in list and time.
        let r = call(&mut core, &tools, "fx_duplicate", json!({ "index": 3 })).unwrap();
        assert_eq!(r["fx_index"], 4);
        assert_eq!(r["start"], 14.0);
        assert_eq!(core.project.fx_clips.len(), 7);
        assert_eq!(core.project.fx_clips[4].clip.start, 14.0);

        // Delete removes exactly the addressed clip.
        let r = call(&mut core, &tools, "fx_delete", json!({ "index": 4 })).unwrap();
        assert_eq!(r["count"], 6);
        assert_eq!(core.project.fx_clips.len(), 6);
        let r = call(&mut core, &tools, "fx_list", json!({})).unwrap();
        assert_eq!(r["count"], 6);
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
