//! Wave 14 (B6) — shell selection tools: MCP handlers that set
//! the GUI's selections exactly as clicking does, and one
//! read-back tool.
//!
//! # Where each selection lives (source evidence)
//!
//! The editor's selections are split across the core and the
//! shell, and this file writes each one where the GUI writes
//! it:
//!
//! * **Figure** — `EditorCore::selected_figure`
//!   (`src/editor.rs:34`). The Figures-panel row press calls
//!   `core.select_figure(fi)` (`src/rune_sec/figures_panel.rs:813`),
//!   which sets the figure AND clears the node selection
//!   (`src/editor.rs:2137-2142`).
//! * **Node** — `EditorCore::selected_node: Option<usize>`
//!   (`src/editor.rs:35`), an index into the selected figure's
//!   `def.nodes`. The only GUI gesture that selects a node is
//!   pressing a joint on the stage, whose write is the pair
//!   `selected_figure = fi; selected_node = Some(ni)`
//!   (`src/editor.rs:378-379`, inside `begin_drag`).
//! * **Sprite** — shell state: `State::sprites_panel.selected`
//!   (`src/main.rs:164`; `SpritesPanelState::selected`,
//!   `src/rune_sec/sprites_panel.rs:173`). The row click is a
//!   pure state write, bounds-checked against
//!   `project.sprites.len()` (`select_sprite`,
//!   `src/rune_sec/sprites_panel.rs:401-405`); its own doc
//!   comment states the project is untouched and nothing is
//!   snapshotted.
//! * **FX clip** — shell state: `State::fx_selected`
//!   (`src/main.rs:180`, documented there as "the core has no
//!   FX-clip selection state"), an index into
//!   `project.fx_clips` (`src/model.rs:774`). Both click
//!   surfaces write `s.fx_selected = Some(i)`: the timeline
//!   lane press (`src/rune_sec/fx_clips.rs:1137`) and the
//!   panel row press (`src/rune_sec/fx_clips.rs:1343`).
//! * **Audio clip** — shell state:
//!   `State::audio_panel.selected` (`AudioPanelState`,
//!   `src/rune_sec/audio_panel.rs:267-277`), an index into
//!   `project.audio`. The clip-row press writes
//!   `s.audio_panel.selected = Some(i)`
//!   (`src/rune_sec/audio_panel.rs:1218`).
//! * **Frame** — the playhead is core state:
//!   `EditorCore::current` + `EditorCore::playhead`
//!   (`src/editor.rs:33,63`), moved together by
//!   `core.goto(frame)` (`src/editor.rs:996-1001`). There is
//!   NO distinct "selected frame" state in the tree (a grep
//!   for `selected_frame` returns no hits): a plain timeline
//!   chip click is `range_sel = None; core.goto(anchor)`
//!   on release (`src/rune_sec/timeline_marks.rs:1159-1163`),
//!   while a drag across chips leaves only the shell-side
//!   range selection `State::frame_ops.range_sel`
//!   (`src/rune_sec/frame_ops.rs:152`) and does not move the
//!   playhead. `shell_select_frame` therefore reproduces the
//!   plain click exactly: validate, `goto`, clear the range
//!   selection.
//!
//! # The no-undo / no-dirty contract
//!
//! Selection changes are NOT undo steps and never mark the
//! project dirty — verified per path above: `select_figure`,
//! `goto`, `select_sprite` and the FX/audio row writes call
//! neither `snapshot()` nor `mark_dirty()`; the shell-side
//! writes touch no core state at all. This matches the
//! standing doctrine that undo stores the Project only and
//! playhead/selection are never rewound.
//!
//! **Discrepancy flagged, not copied:** the stage-press path
//! `begin_drag` (`src/editor.rs:368-379`) calls
//! `self.snapshot()` BEFORE writing the figure/node
//! selection, so in the GUI today a joint press that turns
//! into a drag spends exactly one undo step — but that step
//! belongs to the drag transaction (the pose edits that
//! follow), not to the selection write, which is the plain
//! field pair at `editor.rs:378-379`. `shell_select_node`
//! performs the selection writes only and spends no step.
//! A press released without moving still leaves that
//! snapshot on the GUI's undo stack; that is a GUI-side
//! quirk of the drag path, recorded here rather than
//! reproduced.
//!
//! A second overlap, flagged for the docs pass: the built-in
//! `set_frame` (`src/mcp.rs:382-425`) already validates and
//! calls `core.goto` through the core dispatch.
//! `shell_select_frame` is its shell-channel counterpart
//! (same refusal wording) and additionally clears the
//! timeline range selection, which `set_frame` cannot reach
//! because that state is shell-side.
//!
//! # Splice notes (integrator)
//!
//! * `src/lib.rs`: add `pub mod shell_tools_selection;`.
//! * `tool_shell_select_figure` and `tool_shell_select_node`
//!   take only `&mut EditorCore` and match the registry
//!   handler type (`src/mcp.rs:58-59`) exactly; the other
//!   handlers additionally take [`ShellSelectionTargets`]
//!   (writes) or [`ShellSelectionView`] (read-back), which
//!   the shell-command drain builds from the live `State`
//!   in four lines:
//!
//! ```ignore
//! let mut targets = ShellSelectionTargets {
//!     sprite_selected: &mut state.sprites_panel.selected,
//!     fx_selected: &mut state.fx_selected,
//!     audio_selected: &mut state.audio_panel.selected,
//!     frame_range_sel: &mut state.frame_ops.range_sel,
//!     stroke_selected: &mut state.stroke_selected,
//!     particle_selected: &mut state.particle_selected,
//! };
//! ```
//!
//!   (All four fields are `pub` at their definitions —
//!   `sprites_panel.rs:173`, `main.rs:180`,
//!   `audio_panel.rs:277`, `frame_ops.rs:152`.) For
//!   `shell_get_selection`, build `ShellSelectionView` the
//!   same way with shared references.
//! * These are shell tools: on the stdio server there is no
//!   GUI, so dispatch must refuse them with the honest
//!   no-shell error from the SHELL_CONTROL design rather
//!   than applying them to a headless core.
//! * Every handler validates its index against the live
//!   project counts BEFORE writing anything, and refusals
//!   quote the valid range. No handler snapshots, marks
//!   dirty, or sets `core.status` — a successful call
//!   reports through its JSON result only, exactly as a
//!   click sets no status for selecting.

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;

/// Mutable handles to the shell-side selection fields, built
/// by the shell-command drain from the live shell `State`
/// (see the module doc for the construction and the source
/// anchor of each field).
pub struct ShellSelectionTargets<'a> {
    /// `State::sprites_panel.selected` — index into
    /// `project.sprites`.
    pub sprite_selected: &'a mut Option<usize>,
    /// `State::fx_selected` — index into `project.fx_clips`.
    pub fx_selected: &'a mut Option<usize>,
    /// `State::audio_panel.selected` — index into
    /// `project.audio`.
    pub audio_selected: &'a mut Option<usize>,
    /// `State::frame_ops.range_sel` — the timeline range
    /// selection `(anchor, focus)`.
    pub frame_range_sel: &'a mut Option<(usize, usize)>,
    /// `State::stroke_selected` — index into the
    /// project's stroke list (Wave 14 integrator
    /// addition: the Edit menu's selection-relative
    /// deletes act on it).
    pub stroke_selected: &'a mut Option<usize>,
    /// `State::particle_selected` — index into
    /// `project.particles` (same addition).
    pub particle_selected: &'a mut Option<usize>,
}

/// Shared-reference twin of [`ShellSelectionTargets`] for
/// the read-back tool.
pub struct ShellSelectionView<'a> {
    /// `State::sprites_panel.selected`.
    pub sprite_selected: &'a Option<usize>,
    /// `State::fx_selected`.
    pub fx_selected: &'a Option<usize>,
    /// `State::audio_panel.selected`.
    pub audio_selected: &'a Option<usize>,
    /// `State::frame_ops.range_sel`.
    pub frame_range_sel: &'a Option<(usize, usize)>,
    /// `State::stroke_selected`.
    pub stroke_selected: &'a Option<usize>,
    /// `State::particle_selected`.
    pub particle_selected: &'a Option<usize>,
}

/// Required index argument, with the registry helpers'
/// wording (`mcp_tools::req_usize` / `as_usize`,
/// `src/mcp_tools.rs:67-84`): a non-negative integer, where
/// a whole-valued JSON float is also accepted.
fn req_index(args: &Value, key: &str) -> Result<usize, String> {
    let v = args
        .get(key)
        .filter(|v| !v.is_null())
        .ok_or_else(|| format!("missing required argument '{key}'"))?;
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!(
        "argument '{key}' must be a non-negative integer"
    ))
}

/// Refuse `index` unless it names a live entry, quoting the
/// valid range (the refusal shape of the built-in
/// `set_frame`, `src/mcp.rs:416-420`). `noun` is the thing
/// being selected ("figure", "FX clip", …) and `owner` the
/// phrase naming what holds them ("the project", "the
/// figure").
fn check_index(index: usize, count: usize, noun: &str, owner: &str) -> Result<(), String> {
    if index < count {
        return Ok(());
    }
    if count == 0 {
        Err(format!(
            "{noun} index {index} is out of range: {owner} has no {noun}s to select"
        ))
    } else {
        Err(format!(
            "{noun} index {index} is out of range: {owner} has {count} {noun}(s), \
             valid indices are 0..={}",
            count - 1
        ))
    }
}

// TOOL ENTRY SPEC: shell_select_figure
//   name: "shell_select_figure"
//   description: "Select a figure in the GUI exactly as clicking its row in the Figures panel does: 'figure' (0-based) becomes the selected figure and the node selection is cleared. Shell selection only — no undo step, never marks the project dirty."
//   schema: obj_schema(json!({ "figure": { "type": "integer" } }), &["figure"])
//   handler: crate::shell_tools_selection::tool_shell_select_figure
//     (core-only — matches the registry handler type in src/mcp.rs:58-59;
//     dispatched through the shell-command channel, not all_tools()).
//   MCP.md row:
//   | `shell_select_figure` | Select a figure in the GUI exactly as clicking its Figures-panel row (the node selection is cleared). Shell selection only: no undo step, never marks dirty. | `figure` | — |
/// Select a figure, exactly as the Figures-panel row press
/// does: `core.select_figure` (`src/editor.rs:2137-2142`),
/// which also clears the node selection. No snapshot, no
/// dirty mark — `select_figure` performs neither, and
/// neither does this handler.
pub fn tool_shell_select_figure(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let figure = req_index(args, "figure")?;
    check_index(figure, core.project.figures.len(), "figure", "the project")?;
    core.select_figure(figure);
    Ok(json!({
        "figure": core.selected_figure,
        "node": core.selected_node,
        "figure_count": core.project.figures.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_node
//   name: "shell_select_node"
//   description: "Select node 'node' of figure 'figure' in the GUI exactly as pressing that joint on the stage does: both the figure and the node selection move. Shell selection only — no undo step (the GUI's stage press snapshots for the drag that may follow, not for the selection; see this file's module doc), never marks the project dirty."
//   schema: obj_schema(json!({ "figure": { "type": "integer" }, "node": { "type": "integer" } }), &["figure", "node"])
//   handler: crate::shell_tools_selection::tool_shell_select_node
//     (core-only — matches the registry handler type in src/mcp.rs:58-59;
//     dispatched through the shell-command channel, not all_tools()).
//   MCP.md row:
//   | `shell_select_node` | Select a node of a figure in the GUI exactly as pressing that joint on the stage (figure and node selection both move). Shell selection only: no undo step, never marks dirty. | `figure`, `node` | — |
/// Select a node of a figure, exactly as the stage press
/// writes it (`src/editor.rs:378-379`): the figure and node
/// fields move together. Only the selection writes are
/// performed — `begin_drag`'s snapshot belongs to the drag
/// transaction and is deliberately not reproduced (see the
/// module doc).
pub fn tool_shell_select_node(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let figure = req_index(args, "figure")?;
    check_index(figure, core.project.figures.len(), "figure", "the project")?;
    let node = req_index(args, "node")?;
    let node_count = core.project.figures[figure].def.nodes.len();
    check_index(node, node_count, "node", "the figure")?;
    core.selected_figure = figure;
    core.selected_node = Some(node);
    Ok(json!({
        "figure": figure,
        "node": node,
        "node_count": node_count,
    }))
}

// TOOL ENTRY SPEC: shell_select_sprite
//   name: "shell_select_sprite"
//   description: "Select sprite instance 'sprite' (0-based) in the GUI exactly as clicking its row in the Sprites panel does. Shell selection only — no undo step, never marks the project dirty."
//   schema: obj_schema(json!({ "sprite": { "type": "integer" } }), &["sprite"])
//   handler: crate::shell_tools_selection::tool_shell_select_sprite
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-command drain, not registrable in all_tools() as-is).
//   MCP.md row:
//   | `shell_select_sprite` | Select a sprite instance in the GUI exactly as clicking its Sprites-panel row. Shell selection only: no undo step, never marks dirty. | `sprite` | — |
/// Select a sprite instance, exactly as the Sprites-panel
/// row click writes it (`select_sprite`,
/// `src/rune_sec/sprites_panel.rs:401-405`): a pure
/// shell-state write, bounds-checked there by the row's
/// existence and here by an explicit ranged refusal.
pub fn tool_shell_select_sprite(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let sprite = req_index(args, "sprite")?;
    check_index(sprite, core.project.sprites.len(), "sprite", "the project")?;
    *targets.sprite_selected = Some(sprite);
    Ok(json!({
        "sprite": sprite,
        "sprite_count": core.project.sprites.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_fx_clip
//   name: "shell_select_fx_clip"
//   description: "Select FX clip 'clip' (0-based, an index into the project's FX clips) in the GUI exactly as pressing its bar in the FX lane or its panel row does. Shell selection only — no undo step, never marks the project dirty."
//   schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"])
//   handler: crate::shell_tools_selection::tool_shell_select_fx_clip
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-command drain, not registrable in all_tools() as-is).
//   MCP.md row:
//   | `shell_select_fx_clip` | Select an FX clip in the GUI exactly as pressing its FX-lane bar or panel row. Shell selection only: no undo step, never marks dirty. | `clip` | — |
/// Select an FX clip, exactly as both GUI click surfaces
/// write it (`src/rune_sec/fx_clips.rs:1137` and `:1343`):
/// `State::fx_selected = Some(i)`, a pure shell-state
/// write.
pub fn tool_shell_select_fx_clip(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let clip = req_index(args, "clip")?;
    check_index(clip, core.project.fx_clips.len(), "FX clip", "the project")?;
    *targets.fx_selected = Some(clip);
    Ok(json!({
        "fx_clip": clip,
        "fx_clip_count": core.project.fx_clips.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_audio_clip
//   name: "shell_select_audio_clip"
//   description: "Select audio clip 'clip' (0-based, an index into the project's audio clips) in the GUI exactly as pressing its row in the Audio panel does — the clip the audio operations toolbar targets. Shell selection only — no undo step, never marks the project dirty."
//   schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"])
//   handler: crate::shell_tools_selection::tool_shell_select_audio_clip
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-command drain, not registrable in all_tools() as-is).
//   MCP.md row:
//   | `shell_select_audio_clip` | Select an audio clip in the GUI exactly as pressing its Audio-panel row (the operations toolbar's target). Shell selection only: no undo step, never marks dirty. | `clip` | — |
/// Select an audio clip, exactly as the Audio-panel row
/// press writes it (`src/rune_sec/audio_panel.rs:1218`):
/// `State::audio_panel.selected = Some(i)`, a pure
/// shell-state write.
pub fn tool_shell_select_audio_clip(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let clip = req_index(args, "clip")?;
    check_index(clip, core.project.audio.len(), "audio clip", "the project")?;
    *targets.audio_selected = Some(clip);
    Ok(json!({
        "audio_clip": clip,
        "audio_clip_count": core.project.audio.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_frame
//   name: "shell_select_frame"
//   description: "Move the GUI playhead to frame 'frame' (0-based) exactly as a plain click on that frame's timeline chip does: the playhead moves and any timeline range selection is cleared. Shell selection only — no undo step, never marks the project dirty."
//   schema: obj_schema(json!({ "frame": { "type": "integer" } }), &["frame"])
//   handler: crate::shell_tools_selection::tool_shell_select_frame
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-command drain, not registrable in all_tools() as-is).
//   MCP.md row:
//   | `shell_select_frame` | Move the GUI playhead to a frame exactly as a plain timeline-chip click (any range selection is cleared). Shell selection only: no undo step, never marks dirty. | `frame` | — |
/// Select a frame, exactly as a plain timeline-chip click
/// resolves on release (`src/rune_sec/timeline_marks.rs:1159-1163`):
/// the range selection is cleared and `core.goto` moves
/// `current` and `playhead` together. (A chip drag, by
/// contrast, leaves a range selection and never moves the
/// playhead — that is a range gesture, not a frame
/// selection, and no tool here reproduces it.) Validation
/// is stricter than `goto`'s silent ignore, with the
/// built-in `set_frame`'s refusal shape.
pub fn tool_shell_select_frame(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let frame = req_index(args, "frame")?;
    check_index(frame, core.project.frames.len(), "frame", "the project")?;
    core.goto(frame);
    *targets.frame_range_sel = None;
    Ok(json!({
        "frame": core.current,
        "frame_count": core.project.frames.len(),
        "frame_range": Value::Null,
    }))
}

// TOOL ENTRY SPEC: shell_select_stroke
//   name: "shell_select_stroke"
//   description: "Select a stroke in the GUI — 'stroke' (0-based, an index into the project's stroke list, the index delete_stroke consumes) becomes the shell's stroke selection, the one the Edit menu's Delete stroke acts on. Shell selection only — no undo step, never marks the project dirty. Live endpoint only."
//   schema: obj_schema(json!({ "stroke": { "type": "integer" } }), &["stroke"])
//   handler: crate::shell_tools_selection::tool_shell_select_stroke
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-control drain, not registrable in all_tools()).
//   MCP.md row:
//   | `shell_select_stroke` | Select a stroke on the current frame (the Edit menu's selection). Shell selection only: no undo step, never marks dirty. Live endpoint only. | `stroke` | — |
/// Select a stroke — the write the stroke inspector's
/// row press makes to `State::stroke_selected` (Wave 14
/// integrator addition, closing the B9 menu audit's
/// residual: the Edit menu's Delete stroke acts on this
/// selection, and `delete_stroke` by explicit index
/// already covers the mutation itself). The index
/// addresses the PROJECT's stroke list — the same index
/// `delete_stroke_at` consumes (see the palette's
/// `cmd_delete_stroke` shim) — not a per-frame
/// subsequence.
pub fn tool_shell_select_stroke(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let stroke = req_index(args, "stroke")?;
    check_index(stroke, core.project.strokes.len(), "stroke", "the project")?;
    *targets.stroke_selected = Some(stroke);
    Ok(json!({
        "stroke": stroke,
        "stroke_count": core.project.strokes.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_particle
//   name: "shell_select_particle"
//   description: "Select a particle event in the GUI — 'particle' (0-based, an index into the project's particle events) becomes the shell's particle selection, the one the Edit menu's Delete particle event acts on. Shell selection only — no undo step, never marks the project dirty. Live endpoint only."
//   schema: obj_schema(json!({ "particle": { "type": "integer" } }), &["particle"])
//   handler: crate::shell_tools_selection::tool_shell_select_particle
//     (takes &mut ShellSelectionTargets — dispatched by the
//     shell-control drain, not registrable in all_tools()).
//   MCP.md row:
//   | `shell_select_particle` | Select a particle event (the Edit menu's selection). Shell selection only: no undo step, never marks dirty. Live endpoint only. | `particle` | — |
/// Select a particle event — the write the effects
/// panel's row press makes to `State::particle_selected`
/// (Wave 14 integrator addition, the particle twin of
/// [`tool_shell_select_stroke`]).
pub fn tool_shell_select_particle(
    core: &mut EditorCore,
    targets: &mut ShellSelectionTargets,
    args: &Value,
) -> Result<Value, String> {
    let particle = req_index(args, "particle")?;
    check_index(particle, core.project.particles.len(), "particle event", "the project")?;
    *targets.particle_selected = Some(particle);
    Ok(json!({
        "particle": particle,
        "particle_count": core.project.particles.len(),
    }))
}

// TOOL ENTRY SPEC: shell_select_clip
//   name: "shell_select_clip"
//   description: "Select a movieclip in the GUI — 'clip' (0-based, an index into the project's clips) becomes the clips panel's selection. Shell selection only — no undo step, never marks the project dirty. Live endpoint only."
//   schema: obj_schema(json!({ "clip": { "type": "integer" } }), &["clip"])
//   handler: crate::shell_tools_selection::tool_shell_select_clip
//     (writes through clips_panel::set_clip_selection —
//     dispatched by the shell-control drain, not
//     registrable in all_tools()).
//   MCP.md row:
//   | `shell_select_clip` | Select a movieclip in the clips panel. Shell selection only: no undo step, never marks dirty. Live endpoint only. | `clip` | — |
/// Select a movieclip in the clips panel. The panel's
/// selection lives in a module thread-local (not a
/// `State` field), so this handler validates against the
/// live clip count and writes through the panel's own
/// new accessor (`clips_panel::set_clip_selection`,
/// Wave 14) — the same value the panel's row press sets.
pub fn tool_shell_select_clip(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let clip = req_index(args, "clip")?;
    check_index(clip, core.project.clips.len(), "clip", "the project")?;
    crate::rune_sec::clips_panel::set_clip_selection(Some(clip));
    Ok(json!({
        "clip": clip,
        "clip_count": core.project.clips.len(),
    }))
}

// TOOL ENTRY SPEC: shell_get_selection
//   name: "shell_get_selection"
//   description: "Read back every GUI selection: the selected figure and node, the selected sprite, FX clip and audio clip, the playhead frame, and the timeline range selection if one is active, each with the live count it indexes into. Read-only."
//   schema: obj_schema(json!({}), &[])
//   handler: crate::shell_tools_selection::tool_shell_get_selection
//     (takes &ShellSelectionView — dispatched by the
//     shell-command drain, not registrable in all_tools() as-is).
//   MCP.md row:
//   | `shell_get_selection` | Read back every GUI selection (figure, node, sprite, FX clip, audio clip, playhead frame, timeline range) with the live counts. Read-only. | — | — |
/// Read back every selection this file's tools can write,
/// from the same fields the GUI reads. Selection fields
/// that no click has set report as `null`; the figure and
/// frame are always live values. Read-only: nothing is
/// written, so no undo step and no dirty mark by
/// construction.
pub fn tool_shell_get_selection(
    core: &EditorCore,
    view: &ShellSelectionView,
    _args: &Value,
) -> Result<Value, String> {
    Ok(json!({
        "figure": core.selected_figure,
        "node": core.selected_node,
        "sprite": view.sprite_selected,
        "fx_clip": view.fx_selected,
        "audio_clip": view.audio_selected,
        "stroke": view.stroke_selected,
        "particle": view.particle_selected,
        "clip": crate::rune_sec::clips_panel::clip_selection(),
        "frame": core.current,
        "playhead": core.playhead,
        "frame_range": view
            .frame_range_sel
            .map(|(anchor, focus)| json!([anchor, focus])),
        "figure_count": core.project.figures.len(),
        "sprite_count": core.project.sprites.len(),
        "fx_clip_count": core.project.fx_clips.len(),
        "audio_clip_count": core.project.audio.len(),
        "stroke_count": core.project.strokes.len(),
        "particle_count": core.project.particles.len(),
        "clip_count": core.project.clips.len(),
        "frame_count": core.project.frames.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::{AudioClip, FxClipEntry, SpriteInstance};
    use stykanimate_rs::vfx::{ClipKind, ParticleClip};

    /// The four shell-side selection slots as standalone
    /// locals, wired into a targets struct per call.
    struct Slots {
        sprite: Option<usize>,
        fx: Option<usize>,
        audio: Option<usize>,
        range: Option<(usize, usize)>,
        stroke: Option<usize>,
        particle: Option<usize>,
    }

    impl Slots {
        fn new() -> Self {
            Slots {
                sprite: None,
                fx: None,
                audio: None,
                range: None,
                stroke: None,
                particle: None,
            }
        }

        fn targets(&mut self) -> ShellSelectionTargets<'_> {
            ShellSelectionTargets {
                sprite_selected: &mut self.sprite,
                fx_selected: &mut self.fx,
                audio_selected: &mut self.audio,
                frame_range_sel: &mut self.range,
                stroke_selected: &mut self.stroke,
                particle_selected: &mut self.particle,
            }
        }

        fn view(&self) -> ShellSelectionView<'_> {
            ShellSelectionView {
                sprite_selected: &self.sprite,
                fx_selected: &self.fx,
                audio_selected: &self.audio,
                frame_range_sel: &self.range,
                stroke_selected: &self.stroke,
                particle_selected: &self.particle,
            }
        }
    }

    /// FX entry fixture mirrored from the `fx_ops` tests
    /// (`src/fx_ops.rs:767-782`).
    fn fx_entry() -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start: 0.0,
                duration: 12.0,
                kind: ClipKind::Burst,
                color: 0xFFAA33,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        }
    }

    #[test]
    fn select_figure_writes_the_core_fields_and_clears_the_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        let undo_before = core.undo_depth();
        let out = tool_shell_select_figure(&mut core, &json!({ "figure": 0 }))
            .expect("figure 0 exists in the demo project");
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, None, "a figure click clears the node");
        assert_eq!(out["node"], Value::Null);
        assert_eq!(core.undo_depth(), undo_before, "selection spends no undo step");
    }

    #[test]
    fn select_figure_refuses_out_of_range_quoting_the_valid_range() {
        let mut core = EditorCore::new();
        let count = core.project.figures.len();
        let err = tool_shell_select_figure(&mut core, &json!({ "figure": count }))
            .expect_err("one past the last figure must be refused");
        assert!(
            err.contains(&format!("valid indices are 0..={}", count - 1)),
            "err: {err}"
        );
        let err = tool_shell_select_figure(&mut core, &json!({}))
            .expect_err("a missing argument must be refused");
        assert!(err.contains("missing required argument 'figure'"), "err: {err}");
        let err = tool_shell_select_figure(&mut core, &json!({ "figure": -1 }))
            .expect_err("a negative index must be refused");
        assert!(err.contains("must be a non-negative integer"), "err: {err}");
    }

    #[test]
    fn select_node_writes_figure_and_node_together() {
        let mut core = EditorCore::new();
        let nodes = core.project.figures[0].def.nodes.len();
        let last = nodes - 1;
        let undo_before = core.undo_depth();
        let out = tool_shell_select_node(&mut core, &json!({ "figure": 0, "node": last }))
            .expect("the last node exists");
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, Some(last));
        assert_eq!(out["node_count"].as_u64(), Some(nodes as u64));
        assert_eq!(core.undo_depth(), undo_before, "selection spends no undo step");
        let err = tool_shell_select_node(&mut core, &json!({ "figure": 0, "node": nodes }))
            .expect_err("one past the last node must be refused");
        assert!(
            err.contains(&format!("valid indices are 0..={}", nodes - 1)),
            "err: {err}"
        );
        let err = tool_shell_select_node(&mut core, &json!({ "figure": 9, "node": 0 }))
            .expect_err("a missing figure must be refused first");
        assert!(err.contains("figure index 9 is out of range"), "err: {err}");
    }

    #[test]
    fn select_frame_gotos_and_clears_the_range_selection() {
        let mut core = EditorCore::new();
        let mut slots = Slots::new();
        slots.range = Some((0, 2));
        let last = core.frame_count() - 1;
        let undo_before = core.undo_depth();
        let out = tool_shell_select_frame(
            &mut core,
            &mut slots.targets(),
            &json!({ "frame": last }),
        )
        .expect("the last frame exists");
        assert_eq!(core.current, last);
        assert_eq!(core.playhead, last as f32, "goto moves current and playhead together");
        assert_eq!(slots.range, None, "a plain click leaves no range selection behind");
        assert_eq!(out["frame_range"], Value::Null);
        assert_eq!(core.undo_depth(), undo_before, "selection spends no undo step");
        let err = tool_shell_select_frame(
            &mut core,
            &mut slots.targets(),
            &json!({ "frame": last + 1 }),
        )
        .expect_err("one past the last frame must be refused");
        assert!(err.contains("frame index"), "err: {err}");
    }

    #[test]
    fn select_sprite_refuses_an_empty_project_then_selects() {
        let mut core = EditorCore::new();
        let mut slots = Slots::new();
        let err = tool_shell_select_sprite(
            &mut core,
            &mut slots.targets(),
            &json!({ "sprite": 0 }),
        )
        .expect_err("the demo project has no sprites");
        assert!(err.contains("has no sprites to select"), "err: {err}");
        core.project.sprites.push(SpriteInstance {
            asset: 0,
            keys: Vec::new(),
        });
        let undo_before = core.undo_depth();
        tool_shell_select_sprite(&mut core, &mut slots.targets(), &json!({ "sprite": 0 }))
            .expect("sprite 0 now exists");
        assert_eq!(slots.sprite, Some(0));
        assert_eq!(core.undo_depth(), undo_before, "selection spends no undo step");
    }

    #[test]
    fn select_fx_clip_refuses_an_empty_project_then_selects() {
        let mut core = EditorCore::new();
        let mut slots = Slots::new();
        let err = tool_shell_select_fx_clip(
            &mut core,
            &mut slots.targets(),
            &json!({ "clip": 0 }),
        )
        .expect_err("the demo project has no FX clips");
        assert!(err.contains("has no FX clips to select"), "err: {err}");
        core.project.fx_clips.push(fx_entry());
        tool_shell_select_fx_clip(&mut core, &mut slots.targets(), &json!({ "clip": 0 }))
            .expect("FX clip 0 now exists");
        assert_eq!(slots.fx, Some(0));
    }

    #[test]
    fn select_audio_clip_refuses_an_empty_project_then_selects() {
        let mut core = EditorCore::new();
        let mut slots = Slots::new();
        let err = tool_shell_select_audio_clip(
            &mut core,
            &mut slots.targets(),
            &json!({ "clip": 0 }),
        )
        .expect_err("the demo project has no audio clips");
        assert!(err.contains("has no audio clips to select"), "err: {err}");
        core.project.audio.push(AudioClip {
            path: "clip.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        tool_shell_select_audio_clip(&mut core, &mut slots.targets(), &json!({ "clip": 0 }))
            .expect("audio clip 0 now exists");
        assert_eq!(slots.audio, Some(0));
    }

    #[test]
    fn get_selection_reports_every_field_back() {
        let mut core = EditorCore::new();
        let mut slots = Slots::new();
        core.project.sprites.push(SpriteInstance {
            asset: 0,
            keys: Vec::new(),
        });
        tool_shell_select_node(&mut core, &json!({ "figure": 0, "node": 1 }))
            .expect("node 1 exists");
        tool_shell_select_sprite(&mut core, &mut slots.targets(), &json!({ "sprite": 0 }))
            .expect("sprite 0 exists");
        slots.range = Some((1, 3));
        let out = tool_shell_get_selection(&core, &slots.view(), &json!({}))
            .expect("read-back cannot fail");
        assert_eq!(out["figure"].as_u64(), Some(0));
        assert_eq!(out["node"].as_u64(), Some(1));
        assert_eq!(out["sprite"].as_u64(), Some(0));
        assert_eq!(out["fx_clip"], Value::Null);
        assert_eq!(out["audio_clip"], Value::Null);
        assert_eq!(out["frame"].as_u64(), Some(0));
        assert_eq!(out["frame_range"], json!([1, 3]));
        assert_eq!(out["sprite_count"].as_u64(), Some(1));
        assert_eq!(
            out["frame_count"].as_u64(),
            Some(core.frame_count() as u64)
        );
    }
}
