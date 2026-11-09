//! Particle-event engine ops: the `EditorCore` method
//! family for editing the project's particle EVENTS
//! (`Project::particles`) — add, delete, retime, re-anchor
//! and reposition (Phase G, section 4), plus kind and
//! colour (Phase I, section 6), plus duplicate and frame
//! nudge (Phase J, section 5).
//!
//! Particle events are the one-shot effects the rasterizer
//! draws from `export::particles_at` (a burst / sparkle /
//! smoke puff at a stored stage point, alive for
//! `[frame, frame + 0.7 * fps]`). Before this module the
//! only op that created one was `EditorCore::add_particle`
//! (in `editor.rs`), which always anchors to the current
//! selection; nothing could delete, retime, retarget or
//! move a single event afterwards — the shell could draw
//! events (and Phase F built gizmo geometry for a selected
//! one) but no panel could manage them. These ops are the
//! engine home the new particles panel calls, written in
//! the `fx_ops.rs` / `audio_ops.rs` house style:
//!
//! * An out-of-range event index changes nothing and
//!   takes NO snapshot — it only sets `status`, in the
//!   failure voice of the neighbouring ops
//!   ("… failed: no particle event N.").
//! * An in-range op snapshots exactly once, then writes
//!   the field directly (there is no backend module for
//!   particle events the way `fx_edit` backs `fx_ops`).
//! * Success is reported on `status` with 1-based event
//!   and frame numbers, like `fx_move_clip` /
//!   `audio_move_last`.
//!
//! Defaults for a newly added event: NO file importer
//! constructs `ParticleEvent`s — `pivot.rs`, `nodes.rs`,
//! `nodes_anim.rs` and `media.rs` all initialise
//! `particles: Vec::new()` — so the defaults mirror the
//! one production construction site,
//! `EditorCore::add_particle` (editor.rs): kind `Burst`
//! (index 0 of its `PARTICLE_KINDS` table) and the seed
//! `(len * 2654435761) + 12345` (wrapping), taken from the
//! event count BEFORE the push. The colour is the one
//! deliberate delta: `add_particle` colours from the
//! figure it anchors to; an unanchored event has no
//! figure, so the colour comes from the currently
//! selected figure instead (falling back to white
//! `0xFFFFFF`, the colour of the free events in the
//! `layers.rs` tests, when the project has no figures).
//!
//! Frame arguments are clamped to the project's last
//! frame: an event placed past the end could never be
//! seen (its live window starts after the timeline ends),
//! and `add_particle`'s frame (`self.current`) is in
//! range by construction, so clamping is the same
//! contract for panel-supplied frames.
//!
//! Position vs anchor: the model stores both. The
//! rasterizer renders from the stored `[x, y]` ALWAYS
//! (`particles_at` reads `ev.x` / `ev.y` and never the
//! anchor); the anchor (`figure` + `node`, both stored)
//! is what the shell's gizmo resolves live
//! (`rune_sec/stage_fx.rs::event_anchor`). So
//! `particle_set_position` moves where the effect renders,
//! and `particle_set_anchor` moves where the editor marks
//! it — both are real, distinct fields, and both ops
//! exist.
//!
//! Integrator wiring: declare `pub mod particle_ops;` in
//! `lib.rs` (alphabetical: after `pub mod paint;`, before
//! `pub mod pivot;`). Nothing else is needed — this file
//! only adds an inherent `impl EditorCore` block.

use crate::editor::EditorCore;
use crate::model::{ParticleEvent, ParticleKind};
use serde_json::{json, Value};

impl EditorCore {
    /// Clamp a frame argument to the project's last frame
    /// (0 for a project with no frames — defensive; a real
    /// project always has at least one).
    fn particle_clamp_frame(&self, frame: usize) -> usize {
        frame.min(self.frame_count().saturating_sub(1))
    }

    /// Append a default particle event at `frame` /
    /// `(x, y)`, unanchored (`figure: None`, `node: None`)
    /// — a free stage effect. Defaults mirror
    /// `add_particle` (see the module docs): `Burst` kind,
    /// the selected figure's colour, and the count-based
    /// wrapping seed. `frame` is clamped to the last
    /// frame. One undo step. Returns the new event's
    /// index, so the caller can select what it just made.
    pub fn particle_add_event(&mut self, frame: usize, x: f32, y: f32) -> usize {
        let frame = self.particle_clamp_frame(frame);
        let color = self
            .project
            .figures
            .get(self.selected_figure)
            .map(|f| f.color)
            .unwrap_or(0xFFFFFF);
        let seed = (self.project.particles.len() as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(12345);
        self.snapshot();
        self.project.particles.push(ParticleEvent {
            frame,
            figure: None,
            node: None,
            x,
            y,
            kind: ParticleKind::Burst,
            color,
            seed,
        });
        let index = self.project.particles.len() - 1;
        self.status = format!(
            "Particle event {} added at frame {}.",
            index + 1,
            frame + 1
        );
        index
    }

    /// Delete particle event `index`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_delete_event(&mut self, index: usize) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event delete failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles.remove(index);
        self.status = format!("Particle event {} deleted.", index + 1);
    }

    /// Duplicate particle event `index` (Phase J,
    /// section 5): the copy is inserted immediately after
    /// the original, on the same frame, with every other
    /// field carried over — except the seed. A shared seed
    /// would make the two events draw identical particle
    /// patterns, so the copy takes a fresh one from the
    /// family's count-based formula (see
    /// `particle_add_event`), computed from the event
    /// count before the insert; the formula is a bijection
    /// on the count, so the new seed cannot collide with
    /// one an earlier add already handed out. One undo
    /// step. Returns the copy's index, so the caller can
    /// select what it just made; an out-of-range index
    /// only sets the status and returns `None`.
    pub fn particle_duplicate_event(&mut self, index: usize) -> Option<usize> {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event duplicate failed: no particle event {}.",
                index + 1
            );
            return None;
        }
        let seed = (self.project.particles.len() as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(12345);
        let mut copy = self.project.particles[index].clone();
        copy.seed = seed;
        self.snapshot();
        self.project.particles.insert(index + 1, copy);
        self.status = format!(
            "Particle event {} duplicated as event {}.",
            index + 1,
            index + 2
        );
        Some(index + 1)
    }

    /// Move particle event `index` to `frame`, clamped to
    /// the project's last frame. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_set_frame(&mut self, index: usize, frame: usize) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event frame failed: no particle event {}.",
                index + 1
            );
            return;
        }
        let frame = self.particle_clamp_frame(frame);
        self.snapshot();
        self.project.particles[index].frame = frame;
        self.status = format!(
            "Particle event {} moved to frame {}.",
            index + 1,
            frame + 1
        );
    }

    /// Nudge particle event `index`'s frame by `delta`
    /// frames (Phase J, section 5) — a negative delta
    /// moves the event earlier. The landing frame is
    /// clamped to the timeline at both ends: the lower
    /// end by the saturating arithmetic here, the upper
    /// end by the same `particle_clamp_frame` rule
    /// `particle_set_frame` uses. The status reports the
    /// landing frame in the frame family's voice. One
    /// undo step; an out-of-range index only sets the
    /// status.
    pub fn particle_nudge_frame(&mut self, index: usize, delta: i32) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event frame failed: no particle event {}.",
                index + 1
            );
            return;
        }
        let current = self.project.particles[index].frame;
        let target = (current as i64 + i64::from(delta)).max(0) as usize;
        let frame = self.particle_clamp_frame(target);
        self.snapshot();
        self.project.particles[index].frame = frame;
        self.status = format!(
            "Particle event {} moved to frame {}.",
            index + 1,
            frame + 1
        );
    }

    /// Set particle event `index`'s anchor: figure
    /// `figure`, joint `node` — the model stores BOTH as
    /// `Option`s (unlike `FxClipEntry`, whose anchor is a
    /// plain pair). Pass `figure: None` to clear the anchor
    /// back to a free stage event; the node is then cleared
    /// too, whatever was passed (a node anchor without a
    /// figure is meaningless — every stored free event in
    /// the codebase has `node: None`). A `node: None` with
    /// a figure anchors to the figure's root (the shell's
    /// `event_anchor` reads it as node 0, and the status
    /// reports it as node 0). Only the event index is
    /// bounds-checked — the panel passes the core's
    /// selected figure/joint, valid by construction, the
    /// same contract as `fx_set_anchor`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_set_anchor(
        &mut self,
        index: usize,
        figure: Option<usize>,
        node: Option<usize>,
    ) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event anchor failed: no particle event {}.",
                index + 1
            );
            return;
        }
        let node = if figure.is_some() { node } else { None };
        self.snapshot();
        let event = &mut self.project.particles[index];
        event.figure = figure;
        event.node = node;
        self.status = match figure {
            Some(fi) => format!(
                "Particle event {} anchored to fig {} node {}.",
                index + 1,
                fi + 1,
                node.unwrap_or(0)
            ),
            None => format!("Particle event {} anchor cleared.", index + 1),
        };
    }

    /// Move particle event `index`'s stored stage point to
    /// `(x, y)` — the point the rasterizer renders the
    /// effect from, independent of the anchor (see the
    /// module docs). One undo step; an out-of-range index
    /// only sets the status.
    pub fn particle_set_position(&mut self, index: usize, x: f32, y: f32) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event position failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        let event = &mut self.project.particles[index];
        event.x = x;
        event.y = y;
        self.status = format!(
            "Particle event {} moved to ({x:.0}, {y:.0}).",
            index + 1
        );
    }

    /// Set particle event `index`'s kind (burst / sparkle
    /// / smoke) — the effect shape the rasterizer draws
    /// for the event's live window (Phase I, section 6).
    /// One undo step; an out-of-range index only sets the
    /// status.
    pub fn particle_set_kind(&mut self, index: usize, kind: ParticleKind) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event kind failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles[index].kind = kind;
        self.status = format!(
            "Particle event {} kind set to {kind:?}.",
            index + 1
        );
    }

    /// Set particle event `index`'s colour (`0xRRGGBB`)
    /// — the tint the rasterizer draws every particle of
    /// the event in (Phase I, section 6). The status
    /// wording mirrors `fx_set_color` in `fx_ops.rs`. One
    /// undo step; an out-of-range index only sets the
    /// status.
    pub fn particle_set_color(&mut self, index: usize, color: u32) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event colour failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles[index].color = color;
        self.status = format!(
            "Particle event {} colour set to #{color:06X}.",
            index + 1
        );
    }
}

// ---------------------------------------------------------------------
// MCP handlers (Wave 11, agent B7): the particle-event
// tools the registry in `mcp_tools.rs` was missing.
//
// The registry already carried `particle_duplicate_event`,
// `particle_set_kind` and `particle_set_color`, and the
// `mcp.rs` built-ins carry `particle_add_event` (free,
// unanchored) and `clear_particles` — but the panel's
// delete / frame / anchor / position ops and any way to
// LIST the events had no MCP surface at all. These
// handlers close that gap. They follow the `mcp_tools.rs`
// conventions (0-based indices, arguments validated and
// indices pre-checked BEFORE the op runs, the outcome
// reported through `core.status`, never a panic) but are
// self-contained: the arg helpers are local, distinctly
// named copies, because `mcp_tools.rs`'s helpers are
// private to that module. The integrator registers them
// in `tools()` with the `Tool { .. }` entries in B7's
// report (handler: crate::particle_ops::tool_*).
// ---------------------------------------------------------------------

/// `args[key]`, treating JSON null as absent (the
/// `mcp_tools.rs` `get` convention).
fn particle_mcp_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// A JSON value as a non-negative integer (the
/// `mcp_tools.rs` `as_usize` contract: unsigned ints, or
/// floats with no fraction, in range).
fn particle_mcp_value_usize(v: &Value, key: &str) -> Result<usize, String> {
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

/// Required non-negative integer argument.
fn particle_mcp_usize(args: &Value, key: &str) -> Result<usize, String> {
    match particle_mcp_get(args, key) {
        Some(v) => particle_mcp_value_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Optional non-negative integer argument: absent or
/// null yields `None` (for `particle_set_anchor`, a null
/// `figure` is how the anchor is cleared).
fn particle_mcp_opt_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match particle_mcp_get(args, key) {
        Some(v) => particle_mcp_value_usize(v, key).map(Some),
        None => Ok(None),
    }
}

/// Required integer argument in `i32` range (frame
/// nudge deltas may be negative).
fn particle_mcp_i32(args: &Value, key: &str) -> Result<i32, String> {
    match particle_mcp_get(args, key) {
        Some(v) => {
            if let Some(i) = v.as_i64() {
                return i32::try_from(i)
                    .map_err(|_| format!("argument '{key}' is out of range"));
            }
            if let Some(f) = v.as_f64() {
                if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                    return Ok(f as i32);
                }
            }
            Err(format!("argument '{key}' must be an integer"))
        }
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Required finite number argument (stage coordinates):
/// NaN / infinity are refused — serde_json cannot even
/// carry them (they arrive as null, i.e. "missing"), and
/// a non-finite stored point would poison the
/// rasterizer's geometry.
fn particle_mcp_f32(args: &Value, key: &str) -> Result<f32, String> {
    match particle_mcp_get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) if f.is_finite() => Ok(f as f32),
            _ => Err(format!("argument '{key}' must be a finite number")),
        },
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Index of particle event `idx`, or a clean error when
/// the project has no event there (the same wording as
/// `mcp_tools.rs`'s private `check_particle_index`, so
/// both halves of the surface refuse identically).
fn particle_mcp_index(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.particles.len() {
        Err(format!("no particle event at index {idx}"))
    } else {
        Ok(idx)
    }
}

/// Snake-case kind name — the export/MCP vocabulary for
/// the three kinds (the same strings the particles
/// panel's `kind_name` uses; that helper is bin-crate
/// code, so the mapping is restated here).
fn particle_mcp_kind_name(kind: ParticleKind) -> &'static str {
    match kind {
        ParticleKind::Burst => "burst",
        ParticleKind::Sparkle => "sparkle",
        ParticleKind::Smoke => "smoke",
    }
}

/// Parse a particle kind name, case-insensitively (the
/// same contract and error shape as `mcp_tools.rs`'s
/// private `parse_particle_kind`).
fn particle_mcp_parse_kind(name: &str) -> Result<ParticleKind, String> {
    match name.to_ascii_lowercase().as_str() {
        "burst" => Ok(ParticleKind::Burst),
        "sparkle" => Ok(ParticleKind::Sparkle),
        "smoke" => Ok(ParticleKind::Smoke),
        other => Err(format!(
            "unknown particle kind '{other}' (expected \"burst\", \"sparkle\" or \"smoke\")"
        )),
    }
}

/// One event as JSON, for `particle_list_events`: every
/// stored field, so an MCP client can address, retime or
/// re-anchor events without guessing (before this tool
/// the surface exposed only the event COUNT, via the
/// project stats).
fn particle_mcp_event_json(index: usize, ev: &ParticleEvent) -> Value {
    json!({
        "index": index,
        "frame": ev.frame,
        "kind": particle_mcp_kind_name(ev.kind),
        "color": ev.color,
        "seed": ev.seed,
        "x": ev.x,
        "y": ev.y,
        "figure": ev.figure,
        "node": ev.node,
    })
}

/// MCP `particle_list_events`: every particle event in
/// the project, in list order, with every stored field.
/// Read-only — no snapshot, no status change.
pub fn tool_particle_list_events(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let events: Vec<Value> = core
        .project
        .particles
        .iter()
        .enumerate()
        .map(|(i, ev)| particle_mcp_event_json(i, ev))
        .collect();
    Ok(json!({ "ok": true, "count": events.len(), "events": events }))
}

/// MCP `particle_delete_event`: delete the event at
/// `index` (the Particles panel's Delete, the Delete
/// key and the palette's `delete-particle`). Events
/// after the deleted one shift down one index — the
/// same positional-index contract as the FX and audio
/// tools; call `particle_list_events` again to re-read
/// the list. One undo step.
pub fn tool_particle_delete_event(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = particle_mcp_index(core, particle_mcp_usize(args, "index")?)?;
    core.particle_delete_event(idx);
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

/// MCP `particle_set_frame`: move the event at `index`
/// to 0-based `frame`, clamped to the project's last
/// frame (the op's contract, so an event can never be
/// parked past the end of the timeline); the returned
/// `frame` is where the event actually landed. The
/// panel's frame stepper and its "To playhead" action
/// are both this op. One undo step.
pub fn tool_particle_set_frame(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = particle_mcp_index(core, particle_mcp_usize(args, "index")?)?;
    let frame = particle_mcp_usize(args, "frame")?;
    core.particle_set_frame(idx, frame);
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "frame": core.project.particles[idx].frame,
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

/// MCP `particle_nudge_frame`: move the event at
/// `index` by signed `delta` frames, clamped at both
/// ends of the timeline; the returned `frame` is where
/// the event actually landed. One undo step.
pub fn tool_particle_nudge_frame(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = particle_mcp_index(core, particle_mcp_usize(args, "index")?)?;
    let delta = particle_mcp_i32(args, "delta")?;
    core.particle_nudge_frame(idx, delta);
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "frame": core.project.particles[idx].frame,
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

/// MCP `particle_set_position`: move the event at
/// `index`'s stored stage point to (`x`, `y`) — the
/// point the rasterizer renders the effect from,
/// independent of the anchor (the panel's nudge pad is
/// this op in 4-unit steps). One undo step.
pub fn tool_particle_set_position(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = particle_mcp_index(core, particle_mcp_usize(args, "index")?)?;
    let x = particle_mcp_f32(args, "x")?;
    let y = particle_mcp_f32(args, "y")?;
    core.particle_set_position(idx, x, y);
    let ev = &core.project.particles[idx];
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "x": ev.x,
        "y": ev.y,
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

/// MCP `particle_set_anchor`: anchor the event at
/// `index` to figure `figure`, joint `node` (the
/// panel's "Anchor to figure" / "Anchor to joint"), or
/// clear the anchor back to a free stage event by
/// passing `figure` as null or omitting it (the panel's
/// "Clear anchor"; a `node` passed without a figure is
/// ignored, exactly as the op treats it). A figure with
/// no `node` anchors to the figure's root. The figure
/// (and node, when given) are validated BEFORE the op
/// runs — the panel refuses a stale figure selection
/// the same way — so a bad anchor is a clean error,
/// never a stored dangling reference. One undo step.
pub fn tool_particle_set_anchor(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = particle_mcp_index(core, particle_mcp_usize(args, "index")?)?;
    let figure = particle_mcp_opt_usize(args, "figure")?;
    let node = particle_mcp_opt_usize(args, "node")?;
    if let Some(fi) = figure {
        let def = core
            .project
            .figures
            .get(fi)
            .map(|f| &f.def)
            .ok_or_else(|| format!("no figure at index {fi}"))?;
        if let Some(ni) = node {
            if ni >= def.nodes.len() {
                return Err(format!("figure {fi} has no node at index {ni}"));
            }
        }
    }
    core.particle_set_anchor(idx, figure, node);
    let ev = &core.project.particles[idx];
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "figure": ev.figure,
        "node": ev.node,
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

/// MCP `particle_add_anchored`: add a particle event
/// anchored to the SELECTED figure's selected joint (or
/// its root when no joint is selected), on the current
/// frame, with the joint's current position captured as
/// the event's stored point — `EditorCore::add_particle`,
/// the command palette's "Add particle: Burst /
/// Sparkle" commands. (`particle_add_event` is the other
/// creation shape: a FREE event at an explicit frame and
/// point.) `kind` is "burst", "sparkle" or "smoke"
/// (default "burst"). The selection the op reads is
/// validated first, so a project with no figures — or a
/// stale selection — is a clean error, not a panic in
/// the op's direct indexing. One undo step.
pub fn tool_particle_add_anchored(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let kind = match particle_mcp_get(args, "kind") {
        Some(v) => {
            let name = v
                .as_str()
                .ok_or_else(|| "argument 'kind' must be a string".to_string())?;
            particle_mcp_parse_kind(name)?
        }
        None => ParticleKind::Burst,
    };
    if core.project.figures.is_empty() {
        return Err("project has no figures to anchor a particle event to".to_string());
    }
    let fi = core.selected_figure;
    let fig = core
        .project
        .figures
        .get(fi)
        .ok_or_else(|| format!("no figure at index {fi}"))?;
    if let Some(ni) = core.selected_node {
        if ni >= fig.def.nodes.len() {
            return Err(format!("figure {fi} has no node at index {ni}"));
        }
    }
    // `add_particle` reads the figure's pose on the
    // current frame by direct indexing; a project whose
    // current frame carries no pose for the figure (an
    // import mid-repair) would panic there, so refuse
    // first — the editor's own presence idiom.
    let frame = core
        .project
        .frames
        .get(core.current)
        .ok_or_else(|| "project has no current frame".to_string())?;
    if fi >= frame.poses.len() {
        return Err(format!(
            "figure {fi} has no pose on the current frame to anchor to"
        ));
    }
    // The op's own kind table (editor.rs PARTICLE_KINDS)
    // is indexed in enum declaration order.
    let kind_idx = match kind {
        ParticleKind::Burst => 0,
        ParticleKind::Sparkle => 1,
        ParticleKind::Smoke => 2,
    };
    core.add_particle(kind_idx);
    let idx = core.project.particles.len() - 1;
    Ok(json!({
        "ok": true,
        "particle_index": idx,
        "kind": particle_mcp_kind_name(kind),
        "status": core.status,
        "particles": core.project.particles.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Event construction for tests: distinct frame /
    /// position / seed per event so removals and edits are
    /// identifiable, in the field order of the model.
    fn event(frame: usize, x: f32, seed: u32) -> ParticleEvent {
        ParticleEvent {
            frame,
            figure: None,
            node: None,
            x,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed,
        }
    }

    fn core_with(events: Vec<ParticleEvent>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.particles = events;
        core
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the `fx_ops` pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn add_event_lands_with_add_particle_defaults() {
        let mut core = core_with(vec![]);
        core.project.figures[0].color = 0xFF5D5D;
        core.selected_figure = 0;
        let edits = core.edits_since_autosave();
        let index = core.particle_add_event(1, 120.0, 240.0);
        assert_eq!(index, 0);
        assert_eq!(core.project.particles.len(), 1);
        let ev = &core.project.particles[0];
        assert_eq!(ev.frame, 1);
        assert_eq!(ev.figure, None, "added events are unanchored");
        assert_eq!(ev.node, None);
        assert_eq!((ev.x, ev.y), (120.0, 240.0));
        // The defaults `add_particle` (editor.rs) builds:
        // kind index 0 of PARTICLE_KINDS, the figure's
        // colour, and seed (len * 2654435761) + 12345 with
        // len = 0 before the push.
        assert_eq!(ev.kind, ParticleKind::Burst);
        assert_eq!(ev.color, 0xFF5D5D);
        assert_eq!(ev.seed, 12345);
        assert_eq!(core.status, "Particle event 1 added at frame 2.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn add_event_seed_counts_existing_events_and_frame_clamps() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let index = core.particle_add_event(usize::MAX, 5.0, 6.0);
        assert_eq!(index, 1);
        let ev = &core.project.particles[1];
        // len was 1 before the push:
        // 1 * 2654435761 + 12345 (wrapping), as add_particle.
        assert_eq!(ev.seed, 2654435761u32.wrapping_add(12345));
        assert_eq!(ev.frame, last, "frame clamps to the last frame");
        assert_eq!(
            core.status,
            format!("Particle event 2 added at frame {}.", last + 1)
        );
    }

    #[test]
    fn delete_event_removes_the_right_one_and_undo_restores() {
        let mut core = core_with(vec![
            event(0, 10.0, 1),
            event(1, 20.0, 2),
            event(2, 30.0, 3),
        ]);
        let edits = core.edits_since_autosave();
        core.particle_delete_event(1);
        assert_eq!(core.project.particles.len(), 2);
        assert_eq!(core.project.particles[0].seed, 1);
        assert_eq!(core.project.particles[1].seed, 3, "the middle event went");
        assert_eq!(core.status, "Particle event 2 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The one snapshot is a real undo step.
        core.undo();
        assert_eq!(core.status, "Undone.");
        assert_eq!(core.project.particles.len(), 3);
        assert_eq!(core.project.particles[1].seed, 2);
    }

    #[test]
    fn delete_event_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_delete_event(4);
        assert_eq!(
            core.status,
            "Particle event delete failed: no particle event 5."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_frame_moves_and_clamps_to_the_last_frame() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let edits = core.edits_since_autosave();
        core.particle_set_frame(0, 2);
        assert_eq!(core.project.particles[0].frame, 2);
        assert_eq!(core.status, "Particle event 1 moved to frame 3.");
        core.particle_set_frame(0, usize::MAX);
        assert_eq!(core.project.particles[0].frame, last);
        assert_eq!(
            core.status,
            format!("Particle event 1 moved to frame {}.", last + 1)
        );
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_frame_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_frame(1, 0);
        assert_eq!(
            core.status,
            "Particle event frame failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_anchor_sets_then_clears_and_round_trips() {
        let mut core = core_with(vec![event(0, 50.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_anchor(0, Some(1), Some(3));
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (Some(1), Some(3)));
        // Anchoring does not touch the stored render point.
        assert_eq!((ev.x, ev.y), (50.0, 0.0));
        assert_eq!(
            core.status,
            "Particle event 1 anchored to fig 2 node 3."
        );
        // A figure with no node anchors to the root, node 0.
        core.particle_set_anchor(0, Some(0), None);
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (Some(0), None));
        assert_eq!(
            core.status,
            "Particle event 1 anchored to fig 1 node 0."
        );
        // Clearing the figure clears the node too, even
        // when a node is passed alongside.
        core.particle_set_anchor(0, None, Some(2));
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (None, None));
        assert_eq!(core.status, "Particle event 1 anchor cleared.");
        assert_eq!(core.edits_since_autosave(), edits + 3, "one snapshot per op");
    }

    #[test]
    fn set_anchor_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_anchor(2, Some(0), Some(0));
        assert_eq!(
            core.status,
            "Particle event anchor failed: no particle event 3."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_position_moves_the_stored_render_point() {
        let mut core = core_with(vec![event(0, 10.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_position(0, 300.0, 150.0);
        let ev = &core.project.particles[0];
        assert_eq!((ev.x, ev.y), (300.0, 150.0));
        assert_eq!(core.status, "Particle event 1 moved to (300, 150).");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_position_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_position(1, 0.0, 0.0);
        assert_eq!(
            core.status,
            "Particle event position failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_kind_lands_and_leaves_the_other_fields_alone() {
        let mut core = core_with(vec![event(2, 40.0, 7)]);
        let edits = core.edits_since_autosave();
        core.particle_set_kind(0, ParticleKind::Sparkle);
        let ev = &core.project.particles[0];
        assert_eq!(ev.kind, ParticleKind::Sparkle);
        assert_eq!(core.status, "Particle event 1 kind set to Sparkle.");
        // Only the kind changed: frame, point, seed and
        // colour all keep the fixture's values.
        assert_eq!((ev.frame, ev.x, ev.seed, ev.color), (2, 40.0, 7, 0xFFFFFF));
        core.particle_set_kind(0, ParticleKind::Smoke);
        assert_eq!(core.project.particles[0].kind, ParticleKind::Smoke);
        assert_eq!(core.status, "Particle event 1 kind set to Smoke.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_kind_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_kind(1, ParticleKind::Smoke);
        assert_eq!(
            core.status,
            "Particle event kind failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_color_lands_with_padded_hex_status() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_color(0, 0x0A0B0C);
        let ev = &core.project.particles[0];
        assert_eq!(ev.color, 0x0A0B0C);
        assert_eq!(ev.kind, ParticleKind::Burst, "kind untouched");
        // The status pads to six hex digits, like fx_ops.
        assert_eq!(core.status, "Particle event 1 colour set to #0A0B0C.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_color_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_color(3, 0xFF0000);
        assert_eq!(
            core.status,
            "Particle event colour failed: no particle event 4."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn duplicate_event_lands_after_original_with_fresh_seed() {
        let mut core = core_with(vec![
            event(1, 10.0, 1),
            event(2, 20.0, 2),
        ]);
        // Give the original a distinctive shape in every
        // field the copy must carry over unchanged.
        let original = &mut core.project.particles[0];
        original.figure = Some(0);
        original.node = Some(2);
        original.y = 30.0;
        original.kind = ParticleKind::Sparkle;
        original.color = 0x123456;
        let edits = core.edits_since_autosave();
        let new_index = core.particle_duplicate_event(0);
        assert_eq!(new_index, Some(1), "the copy follows the original");
        assert_eq!(core.project.particles.len(), 3);
        let copy = &core.project.particles[1];
        // Every field but the seed is the original's.
        assert_eq!(copy.frame, 1);
        assert_eq!((copy.figure, copy.node), (Some(0), Some(2)));
        assert_eq!((copy.x, copy.y), (10.0, 30.0));
        assert_eq!(copy.kind, ParticleKind::Sparkle);
        assert_eq!(copy.color, 0x123456);
        // The seed is fresh: the family's count formula at
        // the pre-insert count of 2, as `particle_add_event`
        // derives it — and so not the original's seed.
        assert_eq!(
            copy.seed,
            2u32.wrapping_mul(2654435761).wrapping_add(12345)
        );
        assert_ne!(copy.seed, 1, "the copy must not share the seed");
        // The event that followed the original shifted
        // down one slot, untouched.
        assert_eq!(core.project.particles[2].seed, 2);
        assert_eq!(core.project.particles[2].frame, 2);
        assert_eq!(
            core.status,
            "Particle event 1 duplicated as event 2."
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn duplicate_last_event_appends_the_copy_after_it() {
        let mut core = core_with(vec![event(0, 10.0, 1)]);
        let new_index = core.particle_duplicate_event(0);
        assert_eq!(new_index, Some(1));
        assert_eq!(core.project.particles.len(), 2);
        assert_eq!(core.project.particles[0].seed, 1, "original stays first");
        // Count was 1 before the insert:
        // 1 * 2654435761 + 12345 (wrapping).
        assert_eq!(
            core.project.particles[1].seed,
            2654435761u32.wrapping_add(12345)
        );
        assert_eq!(
            core.status,
            "Particle event 1 duplicated as event 2."
        );
    }

    #[test]
    fn duplicate_event_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        let result = core.particle_duplicate_event(3);
        assert_eq!(result, None);
        assert_eq!(
            core.status,
            "Particle event duplicate failed: no particle event 4."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn nudge_frame_moves_and_clamps_at_both_ends() {
        let mut core = core_with(vec![event(1, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let edits = core.edits_since_autosave();
        core.particle_nudge_frame(0, 2);
        assert_eq!(core.project.particles[0].frame, 3);
        assert_eq!(core.status, "Particle event 1 moved to frame 4.");
        // A nudge past the end lands on the last frame,
        // the `particle_set_frame` clamp.
        core.particle_nudge_frame(0, 100);
        assert_eq!(core.project.particles[0].frame, last);
        assert_eq!(
            core.status,
            format!("Particle event 1 moved to frame {}.", last + 1)
        );
        // A nudge before the start lands on frame 0.
        core.particle_nudge_frame(0, -100);
        assert_eq!(core.project.particles[0].frame, 0);
        assert_eq!(core.status, "Particle event 1 moved to frame 1.");
        assert_eq!(core.edits_since_autosave(), edits + 3, "one snapshot per op");
    }

    #[test]
    fn nudge_frame_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_nudge_frame(1, 5);
        assert_eq!(
            core.status,
            "Particle event frame failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    // ---- Wave 11 MCP handlers (B7) -----------------------------

    #[test]
    fn mcp_list_events_reports_every_stored_field() {
        let mut anchored = event(2, 40.0, 7);
        anchored.figure = Some(1);
        anchored.node = Some(3);
        anchored.kind = ParticleKind::Smoke;
        anchored.color = 0x123456;
        let mut core = core_with(vec![event(0, 10.0, 1), anchored]);
        let edits = core.edits_since_autosave();
        let r = tool_particle_list_events(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(r["count"].as_u64().unwrap(), 2);
        assert_eq!(r["events"][0]["index"].as_u64().unwrap(), 0);
        assert_eq!(r["events"][0]["frame"].as_u64().unwrap(), 0);
        assert_eq!(r["events"][0]["kind"], "burst");
        assert_eq!(r["events"][0]["figure"], serde_json::Value::Null);
        assert_eq!(r["events"][1]["frame"].as_u64().unwrap(), 2);
        assert_eq!(r["events"][1]["kind"], "smoke");
        assert_eq!(r["events"][1]["color"].as_u64().unwrap(), 0x123456);
        assert_eq!(r["events"][1]["seed"].as_u64().unwrap(), 7);
        assert_eq!(r["events"][1]["figure"].as_u64().unwrap(), 1);
        assert_eq!(r["events"][1]["node"].as_u64().unwrap(), 3);
        // Read-only: no snapshot.
        assert_eq!(core.edits_since_autosave(), edits);
    }

    #[test]
    fn mcp_delete_event_shifts_later_indices_down() {
        let mut core = core_with(vec![
            event(0, 10.0, 1),
            event(1, 20.0, 2),
            event(2, 30.0, 3),
        ]);
        let r = tool_particle_delete_event(&mut core, &serde_json::json!({ "index": 0 }))
            .unwrap();
        assert_eq!(r["particles"].as_u64().unwrap(), 2);
        // The old index 1 is now index 0: positional
        // indices, exactly what the tool description says.
        assert_eq!(core.project.particles[0].seed, 2);
        assert_eq!(core.project.particles[1].seed, 3);
        // Out-of-range and missing args are clean errors.
        let err = tool_particle_delete_event(&mut core, &serde_json::json!({ "index": 2 }))
            .unwrap_err();
        assert_eq!(err, "no particle event at index 2");
        assert!(tool_particle_delete_event(&mut core, &serde_json::json!({})).is_err());
        assert_eq!(core.project.particles.len(), 2, "failed calls changed nothing");
    }

    #[test]
    fn mcp_set_frame_and_nudge_report_the_landing_frame() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let r = tool_particle_set_frame(
            &mut core,
            &serde_json::json!({ "index": 0, "frame": 9999 }),
        )
        .unwrap();
        assert_eq!(r["frame"].as_u64().unwrap(), last as u64, "the op clamps; the tool reports the landing");
        let r = tool_particle_nudge_frame(
            &mut core,
            &serde_json::json!({ "index": 0, "delta": -2 }),
        )
        .unwrap();
        assert_eq!(r["frame"].as_u64().unwrap(), (last - 2) as u64);
        // A negative frame / fractional delta are errors,
        // not panics or wraps.
        assert!(tool_particle_set_frame(
            &mut core,
            &serde_json::json!({ "index": 0, "frame": -1 })
        )
        .is_err());
        assert!(tool_particle_nudge_frame(
            &mut core,
            &serde_json::json!({ "index": 0, "delta": 1.5 })
        )
        .is_err());
    }

    #[test]
    fn mcp_set_position_validates_and_lands() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let r = tool_particle_set_position(
            &mut core,
            &serde_json::json!({ "index": 0, "x": 12.5, "y": -4.0 }),
        )
        .unwrap();
        assert_eq!(r["x"].as_f64().unwrap(), 12.5);
        assert_eq!((core.project.particles[0].x, core.project.particles[0].y), (12.5, -4.0));
        // A string coordinate is a clean error.
        assert!(tool_particle_set_position(
            &mut core,
            &serde_json::json!({ "index": 0, "x": "left", "y": 0 })
        )
        .is_err());
    }

    #[test]
    fn mcp_set_anchor_sets_clears_and_validates_first() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let edits = core.edits_since_autosave();
        // A figure index the project does not have is an
        // error BEFORE the op runs: no snapshot, no write.
        let err = tool_particle_set_anchor(
            &mut core,
            &serde_json::json!({ "index": 0, "figure": 9, "node": 0 }),
        )
        .unwrap_err();
        assert_eq!(err, "no figure at index 9");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        assert_eq!(core.project.particles[0].figure, None);
        // A valid anchor lands (figure 0 is the demo
        // figure; node 2 exists on the humanoid rig).
        let r = tool_particle_set_anchor(
            &mut core,
            &serde_json::json!({ "index": 0, "figure": 0, "node": 2 }),
        )
        .unwrap();
        assert_eq!(r["figure"].as_u64().unwrap(), 0);
        assert_eq!(r["node"].as_u64().unwrap(), 2);
        // Clearing: figure null clears the node too.
        let r = tool_particle_set_anchor(
            &mut core,
            &serde_json::json!({ "index": 0, "figure": null }),
        )
        .unwrap();
        assert_eq!(r["figure"], serde_json::Value::Null);
        assert_eq!(r["node"], serde_json::Value::Null);
        assert_eq!(core.project.particles[0].node, None);
    }

    #[test]
    fn mcp_add_anchored_uses_the_selection_and_kind() {
        let mut core = EditorCore::new();
        let r = tool_particle_add_anchored(
            &mut core,
            &serde_json::json!({ "kind": "sparkle" }),
        )
        .unwrap();
        assert_eq!(r["particle_index"].as_u64().unwrap(), 0);
        assert_eq!(r["kind"], "sparkle");
        let ev = &core.project.particles[0];
        assert_eq!(ev.kind, ParticleKind::Sparkle);
        assert_eq!(ev.figure, Some(core.selected_figure), "anchored, not free");
        assert_eq!(ev.frame, core.current);
        // An unknown kind is a clean error, nothing added.
        assert!(tool_particle_add_anchored(
            &mut core,
            &serde_json::json!({ "kind": "laser" })
        )
        .is_err());
        assert_eq!(core.project.particles.len(), 1);
    }
}
