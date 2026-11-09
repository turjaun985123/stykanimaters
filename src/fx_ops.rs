//! FX-clip engine ops: the `EditorCore` method family for
//! editing the project's timed particle clips
//! (`Project::fx_clips`) — move, resize, duplicate, delete,
//! colours and anchor (Phase E, section 9), plus the clip
//! kind setter and the kind cycle order (Phase I,
//! section 7), plus the split op (Phase J, section 6).
//!
//! These ops are the engine home for what the Phase D Rune
//! FX-clips panel (`rune_sec/fx_clips.rs`) used to do inline:
//! every panel handler took an undo snapshot and then called
//! the `fx_edit` backend (or, for colour / end colour /
//! anchor, which have no backend function anywhere, wrote
//! the `Project::fx_clips` field directly after the
//! snapshot). The panel keeps the selection and the views;
//! the mutations now live here, next to `add_fx_clip` and
//! the FX preset ops in `editor.rs`, so the MCP tools and
//! any future UI share one implementation.
//!
//! Every op follows the same contract:
//!
//! * An out-of-range clip index changes nothing and takes
//!   NO snapshot — it only sets `status`, in the failure
//!   voice of `fx_save_preset` ("… failed: no FX clip N.").
//! * An in-range op snapshots exactly once, then delegates
//!   to `fx_edit` (`move_clip` / `resize_clip` /
//!   `duplicate_clip` / `delete_clip`, which own the
//!   clamping: start >= 0, duration >= 0.1). Colour, end
//!   colour, kind, anchor and split have no `fx_edit`
//!   function, so — exactly as the panel did — they write
//!   the fields after the snapshot.
//! * Success is reported on `status` in the panel's voice:
//!   1-based clip and figure numbers, the frame wording of
//!   `add_fx_clip`, and for the anchor the panel's exact
//!   "FX clip N re-anchored to fig F node M." line.
//!
//! Integrator wiring: declare `pub mod fx_ops;` in
//! `lib.rs`. Nothing else is needed — this file only adds
//! an inherent `impl EditorCore` block.
//!
//! Wave 11 (MCP end-to-end) adds two things here:
//! [`EditorCore::fx_set_hidden`], the op the panel's
//! Hide/Show toggle always lacked (the panel snapshots
//! and writes the flag inline, `rune_sec/fx_clips.rs`'s
//! `toggle_selected_hidden`), and the MCP handlers for
//! the two FX surfaces the `mcp_tools.rs` registry was
//! missing — `tool_fx_set_hidden` and
//! `tool_fx_preset_list` — written in this file so their
//! `Tool` entries can point at `crate::fx_ops::tool_*`.
//! The handlers follow the Phase J indexed-tool pattern:
//! every argument is validated (and the clip index
//! pre-checked) before anything mutates, and bad input
//! is a clean `Err`, never a panic.
//!
//! Wave 13 (B4) — dirty-marking audit of this file's
//! whole clip-mutator family, against the Squad B
//! contract (every project mutation takes exactly one
//! snapshot — one undo step, which is also the dirty /
//! autosave count — or calls `EditorCore::mark_dirty`
//! where undoability would be wrong; refusals spend
//! nothing). Verdict: **every mutator in this file
//! already complies, and no `mark_dirty` call is
//! needed here.** `fx_move_clip`, `fx_resize_clip`,
//! `fx_split_clip`, `fx_duplicate_clip`,
//! `fx_delete_clip`, `fx_set_color`, `fx_set_color2`,
//! `fx_set_anchor`, `fx_set_kind` and `fx_set_hidden`
//! each call `snapshot()` exactly once, after their
//! refusal checks and before the write, so each is one
//! undo step and one counted edit; every refusal path
//! (out-of-range index, invalid split point) returns
//! before the snapshot and spends nothing. The tests
//! below pin both halves per op (the family-wide pins
//! are `every_fx_op_is_exactly_one_undo_step` and
//! `every_fx_op_refuses_on_an_empty_project_and_spends_nothing`).
//! The file's non-mutators are correctly outside the
//! contract: `fx_kind_cycle` is pure,
//! `tool_fx_preset_list` is read-only, and
//! `tool_fx_preset_delete` deletes a preset FILE in
//! `./fx-presets/` — it writes no project state, so it
//! takes no snapshot and marks nothing dirty.
//!
//! Wave 14 (A5) — the last three clip properties gain
//! ops and MCP handlers here: [`EditorCore::fx_set_seed`],
//! [`EditorCore::fx_set_angle`] and
//! [`EditorCore::fx_set_length`], with `tool_fx_set_seed`,
//! `tool_fx_set_angle` and `tool_fx_set_length` below.
//! Before this wave `seed` and `angle` were readout-only
//! (the panel inspector prints them as a text line, and
//! `fx_list` reports them, but nothing — panel, op or
//! tool — could change them; `add_fx_clip` hardcodes
//! `angle: 0.0` and derives the seed from the clip
//! count), and `length` could be set only at creation
//! (`add_fx_clip`'s `length` argument). The new ops
//! follow the family contract exactly (bounds check,
//! one snapshot, status in the family's voice), so the
//! Wave 13 B4 verdict above extends to them; the family
//! pins below cover them too.
//!
//! Premise correction, stated plainly: the Wave 12 E8
//! names `clip_set_angle` / `clip_set_root` are **not
//! in this file and are not FX-clip ops** — they are
//! the movieclip-session pose-tweak ops in
//! `src/editor.rs`, documented no-snapshot exceptions
//! (per-angle tweaks would flood the undo history;
//! see their MCP catalogue entries). Under the Squad B
//! contract they are `mark_dirty` candidates, but they
//! belong to the editor.rs owner's census (B2), not to
//! this file; nothing about them is changed here. (At
//! audit time `mark_dirty` did not yet exist in
//! `editor.rs` — B2's seam was still in flight — and
//! this file needs no call to it either way.)

use crate::editor::EditorCore;
use crate::fx_edit;
use crate::vfx::ClipKind;
use serde_json::{json, Value};

/// Display text for a frame position, mirroring the FX
/// panel's `frame_label`: whole frames render without a
/// fraction (`4`), fractional ones with one decimal
/// (`4.5`). Clip starts/durations are `f32`, so the status
/// line must not silently truncate them.
fn frame_label(frame: f32) -> String {
    if frame.fract() == 0.0 {
        format!("{}", frame as i64)
    } else {
        format!("{frame:.1}")
    }
}

/// The shortest a clip — or a split half — may be, in
/// frames: the floor `fx_edit::resize_clip` clamps every
/// duration to, so a clip never collapses to nothing. A
/// split that would leave either half shorter than this
/// is refused rather than clamped, because silently
/// moving the split point would not be the split the
/// caller asked for.
const MIN_CLIP_DURATION: f32 = 0.1;

/// All 27 clip kinds in the one canonical cycle order:
/// the FX browser's presentation order
/// (`rune_sec::fx_browser::FX_KINDS` — the five Wave 15
/// categories Explosion, Energy, Magic, Nature, Particles,
/// each in its row order). The browser lives in the shell
/// crate, which the library cannot import, so the order is
/// mirrored here; the two must be kept in step. This is
/// the order the FX-clips panel's kind prev/next buttons
/// walk.
pub const FX_KIND_ORDER: [ClipKind; 27] = [
    // Explosion
    ClipKind::Burst,
    ClipKind::ShockRing,
    ClipKind::Shard,
    ClipKind::Splash,
    // Energy
    ClipKind::Beam,
    ClipKind::Aura,
    ClipKind::Lightning,
    ClipKind::Prism,
    // Magic
    ClipKind::Sparkle,
    ClipKind::Orbit,
    ClipKind::Galaxy,
    ClipKind::Vortex,
    ClipKind::Fireworks,
    ClipKind::Spiral,
    // Nature
    ClipKind::Leaves,
    ClipKind::Petals,
    ClipKind::Bubbles,
    ClipKind::Aurora,
    // Particles
    ClipKind::Smoke,
    ClipKind::Ember,
    ClipKind::Fountain,
    ClipKind::Trail,
    ClipKind::Confetti,
    ClipKind::Meteor,
    ClipKind::Comet,
    ClipKind::Glitter,
    ClipKind::Wave,
];

/// The kind `dir` steps from `current` in
/// [`FX_KIND_ORDER`], wrapping at both ends: +1 is the
/// next kind, -1 the previous one, 0 is `current` itself,
/// and any larger step wraps as many times as needed.
/// Pure helper for the panel's kind prev/next buttons —
/// the panel applies the result through
/// [`EditorCore::fx_set_kind`], which owns the snapshot
/// and the status line.
pub fn fx_kind_cycle(current: ClipKind, dir: i32) -> ClipKind {
    let len = FX_KIND_ORDER.len() as i32;
    let pos = FX_KIND_ORDER
        .iter()
        .position(|kind| *kind == current)
        .unwrap_or(0) as i32;
    FX_KIND_ORDER[(pos + dir).rem_euclid(len) as usize]
}

impl EditorCore {
    /// Move FX clip `index` by `delta_frames` frames
    /// (negative moves it earlier). The resulting start is
    /// clamped to frame 0 by `fx_edit::move_clip`. One
    /// undo step; an out-of-range index only sets the
    /// status.
    pub fn fx_move_clip(&mut self, index: usize, delta_frames: i32) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX clip move failed: no FX clip {}.", index + 1);
            return;
        };
        let new_start = entry.clip.start + delta_frames as f32;
        self.snapshot();
        fx_edit::move_clip(&mut self.project, index, new_start);
        let start = self.project.fx_clips[index].clip.start;
        self.status = format!(
            "FX clip {} moved to frame {}.",
            index + 1,
            frame_label(start + 1.0)
        );
    }

    /// Resize FX clip `index` by `delta` frames (negative
    /// shortens it). The resulting duration is clamped to
    /// >= 0.1 by `fx_edit::resize_clip`, so a clip never
    /// collapses to nothing. One undo step; an
    /// out-of-range index only sets the status.
    pub fn fx_resize_clip(&mut self, index: usize, delta: f32) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX clip resize failed: no FX clip {}.", index + 1);
            return;
        };
        let new_duration = entry.clip.duration + delta;
        self.snapshot();
        fx_edit::resize_clip(&mut self.project, index, new_duration);
        let duration = self.project.fx_clips[index].clip.duration;
        self.status = format!(
            "FX clip {} resized to {} frames.",
            index + 1,
            frame_label(duration)
        );
    }

    /// Split FX clip `index` into two clips at `at_frame`:
    /// the original keeps `[start, at_frame)` and a new
    /// clip, inserted immediately after it, takes
    /// `[at_frame, start + duration)` with every other
    /// field — kind, figure anchor, node, colours, seed,
    /// angle, length — identical to the original's (unlike
    /// [`Self::fx_duplicate_clip`], whose copy is re-timed
    /// and re-seeded: a split is one clip becoming two, so
    /// the halves must between them be exactly the
    /// original). No `fx_edit` backend exists for split,
    /// so — as for colour and kind — the fields are
    /// written after the snapshot. One undo step.
    ///
    /// Refusals (status only, no snapshot, `false`): the
    /// index is out of range, or `at_frame` is not
    /// strictly inside the clip's span with at least
    /// [`MIN_CLIP_DURATION`] on both sides — splitting at
    /// an edge would be a no-op dressed as a success, and
    /// a half shorter than the resize floor could never
    /// have been created by resizing.
    pub fn fx_split_clip(&mut self, index: usize, at_frame: f32) -> bool {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX clip split failed: no FX clip {}.", index + 1);
            return false;
        };
        let start = entry.clip.start;
        let first = at_frame - start;
        let second = start + entry.clip.duration - at_frame;
        // Both halves at least the minimum also proves
        // `at_frame` strictly inside (start, end): an edge
        // or outside point leaves a zero, negative or NaN
        // half, and NaN fails every `>=` comparison.
        if !(first >= MIN_CLIP_DURATION && second >= MIN_CLIP_DURATION) {
            self.status = format!(
                "FX clip split failed: frame {} is not inside clip {}.",
                frame_label(at_frame + 1.0),
                index + 1
            );
            return false;
        }
        self.snapshot();
        let mut second_half = self.project.fx_clips[index].clone();
        second_half.clip.start = at_frame;
        second_half.clip.duration = second;
        self.project.fx_clips[index].clip.duration = first;
        self.project.fx_clips.insert(index + 1, second_half);
        self.status = format!(
            "FX clip {} split at frame {} into clips {} and {}.",
            index + 1,
            frame_label(at_frame + 1.0),
            index + 1,
            index + 2
        );
        true
    }

    /// Duplicate FX clip `index`: the copy is inserted
    /// right after the original and placed right after it
    /// in time, with a bumped seed (see
    /// `fx_edit::duplicate_clip`). Returns the copy's
    /// index — the panel selects it, as the clip the user
    /// most likely wants to adjust next. One undo step;
    /// an out-of-range index only sets the status and
    /// returns `None`.
    pub fn fx_duplicate_clip(&mut self, index: usize) -> Option<usize> {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip duplicate failed: no FX clip {}.", index + 1);
            return None;
        }
        self.snapshot();
        let new_index = fx_edit::duplicate_clip(&mut self.project, index);
        if let Some(new_index) = new_index {
            self.status = format!(
                "FX clip {} duplicated as clip {}.",
                index + 1,
                new_index + 1
            );
        }
        new_index
    }

    /// Delete FX clip `index`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn fx_delete_clip(&mut self, index: usize) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip delete failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        fx_edit::delete_clip(&mut self.project, index);
        self.status = format!("FX clip {} deleted.", index + 1);
    }

    /// Set FX clip `index`'s primary colour (`0xRRGGBB`).
    /// No `fx_edit` backend exists for colour (the MCP
    /// `add_fx_clip` tool sets it only at creation), so —
    /// as the panel did — the field is written after the
    /// snapshot. One undo step; an out-of-range index
    /// only sets the status.
    pub fn fx_set_color(&mut self, index: usize, color: u32) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip colour failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.color = color;
        self.status = format!("FX clip {} colour set to #{color:06X}.", index + 1);
    }

    /// Set FX clip `index`'s gradient end colour, or clear
    /// it with `None` (solid colour). Same no-backend
    /// pattern as [`Self::fx_set_color`]. One undo step;
    /// an out-of-range index only sets the status.
    pub fn fx_set_color2(&mut self, index: usize, color2: Option<u32>) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip end colour failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.color2 = color2;
        self.status = match color2 {
            Some(color2) => format!(
                "FX clip {} end colour set to #{color2:06X}.",
                index + 1
            ),
            None => format!("FX clip {} end colour cleared.", index + 1),
        };
    }

    /// Re-anchor FX clip `index` to joint `node` of figure
    /// `figure` — the anchoring `add_fx_clip` performs at
    /// creation. Only the clip index is bounds-checked
    /// (the panel passes the core's selected figure/joint,
    /// which are valid by construction). One undo step;
    /// an out-of-range index only sets the status.
    pub fn fx_set_anchor(&mut self, index: usize, figure: usize, node: usize) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip anchor failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        let entry = &mut self.project.fx_clips[index];
        entry.figure = figure;
        entry.node = node;
        self.status = format!(
            "FX clip {} re-anchored to fig {} node {}.",
            index + 1,
            figure + 1,
            node
        );
    }

    /// Set FX clip `index`'s kind — the change the clip
    /// creation paths (`add_fx_clip`, the MCP tools) could
    /// previously make only at creation time. No `fx_edit`
    /// backend exists for kind, so — as the panel did for
    /// colour — the field is written after the snapshot.
    /// The status names the kind in the engine's snake-case
    /// vocabulary (`fx_edit::kind_name`, the same names the
    /// MCP tools accept). One undo step; an out-of-range
    /// index only sets the status.
    pub fn fx_set_kind(&mut self, index: usize, kind: ClipKind) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip kind failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.kind = kind;
        self.status = format!(
            "FX clip {} kind set to {}.",
            index + 1,
            fx_edit::kind_name(kind)
        );
    }

    /// Set FX clip `index`'s hidden flag — the op the
    /// panel's Hide/Show toggle never had: the panel
    /// follows this same no-backend pattern inline
    /// (`toggle_selected_hidden` in `rune_sec/fx_clips.rs`
    /// snapshots, writes `entry.hidden`, sets the status),
    /// and this op gives the MCP `fx_set_hidden` tool the
    /// one implementation to delegate to. A hidden clip is
    /// skipped by the stage and by export renderers but
    /// stays in the list, editable — hiding is not
    /// deleting. The success status is the panel toggle's
    /// exact wording ("FX clip N hidden." / "FX clip N
    /// shown."). One undo step; an out-of-range index only
    /// sets the status.
    pub fn fx_set_hidden(&mut self, index: usize, hidden: bool) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip visibility failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].hidden = hidden;
        self.status = format!(
            "FX clip {} {}.",
            index + 1,
            if hidden { "hidden" } else { "shown" }
        );
    }

    /// Set FX clip `index`'s particle random seed — the
    /// value `fx_list` reports and the panel inspector
    /// prints, which before Wave 14 nothing could change
    /// after creation (creation derives it from the clip
    /// count). The same kind with a different seed draws
    /// a different particle pattern, so this is the
    /// "make it look different" knob. No `fx_edit`
    /// backend exists for the seed, so — as for colour
    /// and kind — the field is written after the
    /// snapshot. One undo step; an out-of-range index
    /// only sets the status.
    pub fn fx_set_seed(&mut self, index: usize, seed: u32) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip seed failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.seed = seed;
        self.status = format!("FX clip {} seed set to {seed}.", index + 1);
    }

    /// Set FX clip `index`'s emission angle in degrees —
    /// the directed kinds (beam, trail, fountain, …)
    /// emit along it. Before Wave 14 the angle was
    /// readout-only: every creation path hardcoded
    /// `0.0`, so no clip could ever be aimed. No
    /// `fx_edit` backend exists for the angle, so the
    /// field is written after the snapshot, as for
    /// colour and kind. Any finite angle is stored as
    /// given (the renderers read it modulo a full turn);
    /// the handler owns the finiteness check. One undo
    /// step; an out-of-range index only sets the status.
    pub fn fx_set_angle(&mut self, index: usize, angle: f32) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip angle failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.angle = angle;
        self.status = format!(
            "FX clip {} angle set to {}°.",
            index + 1,
            frame_label(angle)
        );
    }

    /// Set FX clip `index`'s length in stage units —
    /// the beam reach / trail span `add_fx_clip` could
    /// previously set only at creation (its `length`
    /// argument, default 130). No `fx_edit` backend
    /// exists for the length, so the field is written
    /// after the snapshot, as for colour and kind. Stored
    /// as given, exactly as creation stores it; the
    /// handler owns the finiteness check. One undo step;
    /// an out-of-range index only sets the status.
    pub fn fx_set_length(&mut self, index: usize, length: f32) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip length failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.length = length;
        self.status = format!(
            "FX clip {} length set to {}.",
            index + 1,
            frame_label(length)
        );
    }
}

// ---------------------------------------------------------------------
// Wave 11 MCP handlers: the FX surfaces the mcp_tools.rs
// registry was missing — setting a clip's hidden flag and
// listing the saved presets. The argument helpers are
// LOCAL copies of the mcp_tools.rs conventions (same
// messages, same null-means-absent rule) with fx_ names,
// because the originals are private to that module; keep
// the two in step. Both handlers validate everything
// before anything mutates and return Err on bad input —
// never a panic, whatever JSON arrives.
// ---------------------------------------------------------------------

/// One argument of the MCP call: `null` counts as absent,
/// exactly as in `mcp_tools.rs`.
fn fx_arg<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// Required non-negative integer argument, MCP convention:
/// a JSON `u64`, or an integral `f64` >= 0.
fn fx_arg_usize(args: &Value, key: &str) -> Result<usize, String> {
    let Some(v) = fx_arg(args, key) else {
        return Err(format!("missing required argument '{key}'"));
    };
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

/// Required boolean argument, MCP convention.
fn fx_arg_bool(args: &Value, key: &str) -> Result<bool, String> {
    match fx_arg(args, key) {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Required finite `f32` argument, MCP convention
/// (Wave 14): the value must be a finite number AND
/// survive the cast to `f32` finite — a finite `f64`
/// like 1e300 casts to `f32` infinity, which the
/// standing finiteness contract refuses (the
/// `mcp_tools.rs` `finite_f32` rule, restated locally).
fn fx_arg_f32(args: &Value, key: &str) -> Result<f32, String> {
    match fx_arg(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => {
                let cast = f as f32;
                if f.is_finite() && cast.is_finite() {
                    Ok(cast)
                } else {
                    Err(format!("argument '{key}' must be a finite number"))
                }
            }
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Required `u32` argument, MCP convention (Wave 14):
/// a non-negative integer (the `fx_arg_usize` rule)
/// that also fits in a `u32`.
fn fx_arg_u32(args: &Value, key: &str) -> Result<u32, String> {
    let n = fx_arg_usize(args, key)?;
    u32::try_from(n).map_err(|_| format!("argument '{key}' is too large"))
}

/// MCP handler for `fx_set_hidden`: set FX clip `index`'s
/// hidden flag to `hidden`. The index is pre-checked (the
/// Phase J `check_fx_index` convention — a bad index is a
/// clean Err, not just a status line), both arguments are
/// read before the op runs, and the reply reads the flag
/// back from the project.
pub fn tool_fx_set_hidden(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = fx_arg_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no FX clip at index {idx}"));
    }
    let hidden = fx_arg_bool(args, "hidden")?;
    core.fx_set_hidden(idx, hidden);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "hidden": core.project.fx_clips[idx].hidden,
        "status": core.status,
    }))
}

/// MCP handler for `fx_preset_list`: the slugs of every
/// FX preset saved in `./fx-presets/`, sorted — the read
/// side of `fx_preset_save` / `fx_preset_add` that the
/// registry never had (the panel lists them through
/// `EditorCore::fx_preset_names`; an MCP client had no
/// way to discover a preset's name). Read-only: nothing
/// mutates, no snapshot.
pub fn tool_fx_preset_list(_core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let presets = fx_edit::list_clip_presets();
    Ok(json!({
        "ok": true,
        "count": presets.len(),
        "presets": presets,
    }))
}

/// MCP handler for `fx_preset_delete`: remove a saved FX
/// clip preset by name (the CRUD corner — `fx_preset_save`
/// could create them and nothing could delete one). A file
/// operation, not an undo step. Wave 11 (C4), integrator-
/// authored in this file's style.
pub fn tool_fx_preset_delete(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let _ = core;
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n,
        _ => return Err("missing required argument 'name'".to_string()),
    };
    let path = crate::fx_edit::delete_clip_preset(name)?;
    Ok(json!({
        "ok": true,
        "preset": name,
        "path": path.to_string_lossy(),
    }))
}

// ---------------------------------------------------------------------
// Wave 14 MCP handlers (A5): the three clip properties
// that were readout-only — seed, angle, length. Each
// handler validates everything (index pre-checked, the
// value read and finiteness-checked) before the op
// runs, spends exactly the op's one snapshot, and
// reads the stored value back in the reply. The
// TOOL ENTRY SPEC above each handler is the registry
// entry + MCP.md row the integrator splices verbatim.
// ---------------------------------------------------------------------

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside fx_set_hidden.
//   Tool {
//       name: "fx_set_seed",
//       description: "Set an FX clip's particle random seed (the value fx_list reports): the same kind with a different seed draws a different particle pattern. One undo step.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "seed": { "type": "integer", "description": "Particle random seed (0 to 4294967295)" }
//           }),
//           &["index", "seed"],
//       ),
//       handler: crate::fx_ops::tool_fx_set_seed,
//   }
// MCP.md row:
//   | `fx_set_seed` | Set an FX clip's particle random seed — the value `fx_list` reports; the same kind with a different seed draws a different particle pattern. One undo step. | `index`, `seed` | — |
/// MCP handler for `fx_set_seed`: set FX clip `index`'s
/// particle random seed to `seed`.
pub fn tool_fx_set_seed(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = fx_arg_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no FX clip at index {idx}"));
    }
    let seed = fx_arg_u32(args, "seed")?;
    core.fx_set_seed(idx, seed);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "seed": core.project.fx_clips[idx].clip.seed,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside fx_set_hidden.
//   Tool {
//       name: "fx_set_angle",
//       description: "Set an FX clip's emission angle in degrees (the value fx_list reports; the directed kinds — beam, trail, fountain — emit along it). One undo step.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "angle": { "type": "number", "description": "Emission angle in degrees" }
//           }),
//           &["index", "angle"],
//       ),
//       handler: crate::fx_ops::tool_fx_set_angle,
//   }
// MCP.md row:
//   | `fx_set_angle` | Set an FX clip's emission angle in degrees — the value `fx_list` reports; the directed kinds (beam, trail, fountain) emit along it. One undo step. | `index`, `angle` | — |
/// MCP handler for `fx_set_angle`: set FX clip `index`'s
/// emission angle (degrees) to `angle`.
pub fn tool_fx_set_angle(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = fx_arg_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no FX clip at index {idx}"));
    }
    let angle = fx_arg_f32(args, "angle")?;
    core.fx_set_angle(idx, angle);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "angle": core.project.fx_clips[idx].clip.angle,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside fx_set_hidden.
//   Tool {
//       name: "fx_set_length",
//       description: "Set an FX clip's length in stage units — beam reach / trail span (the value add_fx_clip's 'length' sets at creation and fx_list reports). One undo step.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "length": { "type": "number", "description": "Beam reach / trail span in stage units" }
//           }),
//           &["index", "length"],
//       ),
//       handler: crate::fx_ops::tool_fx_set_length,
//   }
// MCP.md row:
//   | `fx_set_length` | Set an FX clip's length in stage units — beam reach / trail span (the value `add_fx_clip`'s `length` sets at creation and `fx_list` reports). One undo step. | `index`, `length` | — |
/// MCP handler for `fx_set_length`: set FX clip `index`'s
/// length (stage units) to `length`.
pub fn tool_fx_set_length(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = fx_arg_usize(args, "index")?;
    if idx >= core.project.fx_clips.len() {
        return Err(format!("no FX clip at index {idx}"));
    }
    let length = fx_arg_f32(args, "length")?;
    core.fx_set_length(idx, length);
    Ok(json!({
        "ok": true,
        "fx_index": idx,
        "length": core.project.fx_clips[idx].clip.length,
        "status": core.status,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FxClipEntry;
    use crate::vfx::{ClipKind, ParticleClip};

    /// Clip construction mirrored from the `fx_clips.rs`
    /// panel tests: entries are pushed straight onto
    /// `Project::fx_clips` because `add_fx_clip` needs a
    /// selected joint, which is not what is under test.
    fn entry(start: f32, duration: f32, kind: ClipKind, color: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind,
                color,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 1,
            node: 5,
            hidden: false,
        }
    }

    fn core_with(clips: Vec<FxClipEntry>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fx_clips = clips;
        core
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted.
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn move_clip_shifts_start_clamps_at_zero_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_move_clip(0, 3);
        assert_eq!(core.project.fx_clips[0].clip.start, 7.0);
        // Status names the 1-based first covered frame.
        assert_eq!(core.status, "FX clip 1 moved to frame 8.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Moving earlier past frame 0 clamps (fx_edit::move_clip).
        core.fx_move_clip(0, -20);
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
        assert_eq!(core.status, "FX clip 1 moved to frame 1.");
        assert_eq!(core.edits_since_autosave(), edits + 2);
    }

    #[test]
    fn move_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_move_clip(5, 3);
        assert_eq!(core.status, "FX clip move failed: no FX clip 6.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn resize_clip_applies_delta_clamps_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_resize_clip(0, -4.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 6.0);
        assert_eq!(core.status, "FX clip 1 resized to 6 frames.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // A delta that would collapse the clip clamps to 0.1.
        core.fx_resize_clip(0, -100.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 0.1);
        assert_eq!(core.edits_since_autosave(), edits + 2);
    }

    #[test]
    fn resize_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_resize_clip(1, 2.0);
        assert_eq!(core.status, "FX clip resize failed: no FX clip 2.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn split_clip_halves_sum_clone_fields_and_snapshots_once() {
        // An entry with distinctive values in every field,
        // so the clone check below cannot pass by accident.
        let original = FxClipEntry {
            clip: ParticleClip {
                start: 4.0,
                duration: 10.0,
                kind: ClipKind::Beam,
                color: 0x5DB2FF,
                color2: Some(0xFF7BAC),
                seed: 42,
                angle: 30.0,
                length: 200.0,
            },
            figure: 2,
            node: 3,
            hidden: false,
        };
        let mut core = core_with(vec![
            entry(0.0, 2.0, ClipKind::Burst, 0xFF5D5D),
            original,
            entry(50.0, 5.0, ClipKind::Smoke, 0x9AA3B2),
        ]);
        let edits = core.edits_since_autosave();
        assert!(core.fx_split_clip(1, 9.0));
        assert_eq!(core.project.fx_clips.len(), 4);
        let first = &core.project.fx_clips[1];
        let second = &core.project.fx_clips[2];
        // The original keeps [4, 9); the new clip, inserted
        // immediately after it, takes [9, 14).
        assert_eq!(first.clip.start, 4.0);
        assert_eq!(first.clip.duration, 5.0);
        assert_eq!(second.clip.start, 9.0);
        assert_eq!(second.clip.duration, 5.0);
        assert_eq!(
            first.clip.duration + second.clip.duration,
            10.0,
            "the halves sum to the original duration"
        );
        // The second half is the original in every other
        // field — kind, colours, seed, angle, length and
        // the figure anchor all cloned, none re-derived.
        assert_eq!(second.clip.kind, ClipKind::Beam);
        assert_eq!(second.clip.color, 0x5DB2FF);
        assert_eq!(second.clip.color2, Some(0xFF7BAC));
        assert_eq!(second.clip.seed, 42);
        assert_eq!(second.clip.angle, 30.0);
        assert_eq!(second.clip.length, 200.0);
        assert_eq!(second.figure, 2);
        assert_eq!(second.node, 3);
        // The neighbours are untouched.
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::Burst);
        assert_eq!(core.project.fx_clips[3].clip.start, 50.0);
        assert_eq!(core.status, "FX clip 2 split at frame 10 into clips 2 and 3.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn split_clip_fractional_point_and_minimum_halves() {
        // A fractional split point lands exactly where
        // asked — clip times are f32 throughout.
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(core.fx_split_clip(0, 4.5));
        assert_eq!(core.project.fx_clips[0].clip.duration, 0.5);
        assert_eq!(core.project.fx_clips[1].clip.start, 4.5);
        assert_eq!(core.project.fx_clips[1].clip.duration, 9.5);
        assert_eq!(core.status, "FX clip 1 split at frame 5.5 into clips 1 and 2.");
        // A half of exactly the 0.1 resize floor is
        // allowed, on either side of the split.
        let mut core = core_with(vec![entry(0.0, 2.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(core.fx_split_clip(0, 1.9), "second half is 0.1 frames");
        let mut core = core_with(vec![entry(0.0, 2.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(core.fx_split_clip(0, 0.1), "first half is 0.1 frames");
    }

    #[test]
    fn split_clip_refusals_change_nothing() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        // Out-of-range index, in the family's failure voice.
        assert!(!core.fx_split_clip(3, 9.0));
        assert_eq!(core.status, "FX clip split failed: no FX clip 4.");
        // Both edges: splitting there is a no-op, not a split.
        assert!(!core.fx_split_clip(0, 4.0), "at the start edge");
        assert_eq!(
            core.status,
            "FX clip split failed: frame 5 is not inside clip 1."
        );
        assert!(!core.fx_split_clip(0, 14.0), "at the end edge");
        // Outside the span entirely.
        assert!(!core.fx_split_clip(0, 2.0));
        assert!(!core.fx_split_clip(0, 20.0));
        // Inside, but a half would fall below the 0.1 floor.
        assert!(!core.fx_split_clip(0, 4.05), "first half would be 0.05");
        assert!(!core.fx_split_clip(0, 13.95), "second half would be 0.05");
        // A NaN split point is inside nothing.
        assert!(!core.fx_split_clip(0, f32::NAN));
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        assert_eq!(core.project.fx_clips.len(), 1);
    }

    #[test]
    fn split_clip_undo_restores_the_single_clip() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(core.fx_split_clip(0, 9.0));
        assert_eq!(core.project.fx_clips.len(), 2);
        // The one snapshot is a real undo step: a single
        // undo rejoins the halves.
        core.undo();
        assert_eq!(core.status, "Undone.");
        assert_eq!(core.project.fx_clips.len(), 1);
        assert_eq!(core.project.fx_clips[0].clip.start, 4.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 10.0);
    }

    #[test]
    fn duplicate_clip_returns_copy_index_and_snapshots_once() {
        let mut core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Bubbles, 0xFFC65D),
            entry(50.0, 5.0, ClipKind::Burst, 0xFF5D5D),
        ]);
        let edits = core.edits_since_autosave();
        let new_index = core.fx_duplicate_clip(0).expect("valid index");
        assert_eq!(new_index, 1);
        assert_eq!(core.project.fx_clips.len(), 3);
        // The copy sits right after the original in time
        // (fx_edit::duplicate_clip), the neighbour shifted.
        assert_eq!(core.project.fx_clips[1].clip.start, 14.0);
        assert_eq!(core.project.fx_clips[2].clip.start, 50.0);
        assert_eq!(core.status, "FX clip 1 duplicated as clip 2.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn duplicate_clip_out_of_range_returns_none_and_changes_nothing() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        assert_eq!(core.fx_duplicate_clip(9), None);
        assert_eq!(core.status, "FX clip duplicate failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn delete_clip_removes_snapshots_once_and_undo_restores() {
        let mut core = core_with(vec![
            entry(0.0, 5.0, ClipKind::Burst, 0xFF5D5D),
            entry(10.0, 5.0, ClipKind::Smoke, 0x9AA3B2),
            entry(20.0, 5.0, ClipKind::Aura, 0xC792FF),
        ]);
        let edits = core.edits_since_autosave();
        core.fx_delete_clip(1);
        assert_eq!(core.project.fx_clips.len(), 2);
        assert_eq!(core.project.fx_clips[1].clip.kind, ClipKind::Aura);
        assert_eq!(core.status, "FX clip 2 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The one snapshot is a real undo step.
        core.undo();
        assert_eq!(core.status, "Undone.");
        assert_eq!(core.project.fx_clips.len(), 3);
        assert_eq!(core.project.fx_clips[1].clip.kind, ClipKind::Smoke);
    }

    #[test]
    fn delete_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_delete_clip(3);
        assert_eq!(core.status, "FX clip delete failed: no FX clip 4.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // Same on an empty list.
        let mut empty = core_with(vec![]);
        empty.fx_delete_clip(0);
        assert_eq!(empty.status, "FX clip delete failed: no FX clip 1.");
        assert!(empty.project.fx_clips.is_empty());
    }

    #[test]
    fn set_color_writes_colour_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_color(0, 0x5DB2FF);
        assert_eq!(core.project.fx_clips[0].clip.color, 0x5DB2FF);
        assert_eq!(core.status, "FX clip 1 colour set to #5DB2FF.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_color2_sets_then_clears_and_snapshots_each_time() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_color2(0, Some(0xFF7BAC));
        assert_eq!(core.project.fx_clips[0].clip.color2, Some(0xFF7BAC));
        assert_eq!(core.status, "FX clip 1 end colour set to #FF7BAC.");
        core.fx_set_color2(0, None);
        assert_eq!(core.project.fx_clips[0].clip.color2, None);
        assert_eq!(core.status, "FX clip 1 end colour cleared.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_anchor_uses_the_panels_exact_wording() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_anchor(0, 2, 4);
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.fx_clips[0].node, 4);
        // Byte-identical to the Phase D panel's re-anchor line.
        assert_eq!(core.status, "FX clip 1 re-anchored to fig 3 node 4.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn colour_and_anchor_ops_out_of_range_are_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_set_color(9, 0xFFFFFF);
        assert_eq!(core.status, "FX clip colour failed: no FX clip 10.");
        core.fx_set_color2(9, Some(0xFFFFFF));
        assert_eq!(core.status, "FX clip end colour failed: no FX clip 10.");
        core.fx_set_anchor(9, 0, 0);
        assert_eq!(core.status, "FX clip anchor failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_kind_writes_kind_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_kind(0, ClipKind::ShockRing);
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::ShockRing);
        // The status uses the engine's snake-case kind name.
        assert_eq!(core.status, "FX clip 1 kind set to shock_ring.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_kind_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_set_kind(9, ClipKind::Wave);
        assert_eq!(core.status, "FX clip kind failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn kind_cycle_steps_in_browser_order_and_wraps() {
        // Within a category.
        assert_eq!(fx_kind_cycle(ClipKind::Burst, 1), ClipKind::ShockRing);
        assert_eq!(fx_kind_cycle(ClipKind::ShockRing, -1), ClipKind::Burst);
        // Across category boundaries (Explosion -> Energy,
        // Nature -> Particles).
        assert_eq!(fx_kind_cycle(ClipKind::Splash, 1), ClipKind::Beam);
        assert_eq!(fx_kind_cycle(ClipKind::Aurora, 1), ClipKind::Smoke);
        // Wrap at both ends of the whole order.
        assert_eq!(fx_kind_cycle(ClipKind::Wave, 1), ClipKind::Burst);
        assert_eq!(fx_kind_cycle(ClipKind::Burst, -1), ClipKind::Wave);
        // No step and whole turns stay put; bigger steps wrap.
        assert_eq!(fx_kind_cycle(ClipKind::Sparkle, 0), ClipKind::Sparkle);
        assert_eq!(fx_kind_cycle(ClipKind::Meteor, 27), ClipKind::Meteor);
        assert_eq!(fx_kind_cycle(ClipKind::Meteor, -27), ClipKind::Meteor);
        assert_eq!(fx_kind_cycle(ClipKind::Burst, 2), ClipKind::Shard);
    }

    #[test]
    fn kind_cycle_order_covers_every_kind_exactly_once() {
        assert_eq!(FX_KIND_ORDER.len(), 27);
        for kind in FX_KIND_ORDER {
            let count = FX_KIND_ORDER.iter().filter(|k| **k == kind).count();
            assert_eq!(count, 1, "kind {kind:?} appears {count} times");
        }
        // Walking +1 from the first kind visits 27 distinct
        // kinds and lands back at the start.
        let mut visited: Vec<ClipKind> = Vec::new();
        let mut kind = FX_KIND_ORDER[0];
        for _ in 0..27 {
            assert!(!visited.contains(&kind), "kind {kind:?} visited twice");
            visited.push(kind);
            kind = fx_kind_cycle(kind, 1);
        }
        assert_eq!(kind, FX_KIND_ORDER[0], "a full turn returns to the start");
    }

    #[test]
    fn set_hidden_writes_the_flag_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_hidden(0, true);
        assert!(core.project.fx_clips[0].hidden);
        // The panel toggle's exact wording.
        assert_eq!(core.status, "FX clip 1 hidden.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        core.fx_set_hidden(0, false);
        assert!(!core.project.fx_clips[0].hidden);
        assert_eq!(core.status, "FX clip 1 shown.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
        // The one snapshot is a real undo step.
        core.undo();
        assert!(core.project.fx_clips[0].hidden, "undo restores the hidden flag");
    }

    #[test]
    fn set_hidden_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_set_hidden(9, true);
        assert_eq!(core.status, "FX clip visibility failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // Same on an empty list.
        let mut empty = core_with(vec![]);
        empty.fx_set_hidden(0, true);
        assert_eq!(empty.status, "FX clip visibility failed: no FX clip 1.");
    }

    /// Wave 13 (B4) family pin helper: applying `apply`
    /// to `core` must take exactly one snapshot (one
    /// counted edit for the dirty dot / autosave
    /// cadence), and a single `undo()` must restore the
    /// project byte-for-byte — together those prove the
    /// snapshot is a real undo step, not just a counter
    /// bump.
    fn assert_single_undo_step(
        core: &mut EditorCore,
        label: &str,
        apply: impl FnOnce(&mut EditorCore),
    ) {
        let before = project_json(core);
        let edits = core.edits_since_autosave();
        apply(core);
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "{label}: exactly one snapshot"
        );
        core.undo();
        assert_eq!(core.status, "Undone.", "{label}: undo is available");
        assert_eq!(
            project_json(core),
            before,
            "{label}: one undo restores the project exactly"
        );
    }

    #[test]
    fn every_fx_op_is_exactly_one_undo_step() {
        // The B4 audit as an executable census: every
        // mutating op in this file, one after another on
        // the same fixture, each pinned by
        // `assert_single_undo_step`. (Split, delete and
        // set-hidden also have dedicated undo tests
        // above; they are repeated here so the family
        // pin has no silent exceptions.)
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        assert_single_undo_step(&mut core, "fx_move_clip", |c| c.fx_move_clip(0, 3));
        assert_single_undo_step(&mut core, "fx_resize_clip", |c| c.fx_resize_clip(0, -4.0));
        assert_single_undo_step(&mut core, "fx_split_clip", |c| {
            assert!(c.fx_split_clip(0, 9.0));
        });
        assert_single_undo_step(&mut core, "fx_duplicate_clip", |c| {
            assert_eq!(c.fx_duplicate_clip(0), Some(1));
        });
        assert_single_undo_step(&mut core, "fx_delete_clip", |c| c.fx_delete_clip(0));
        assert_single_undo_step(&mut core, "fx_set_color", |c| c.fx_set_color(0, 0x5DB2FF));
        assert_single_undo_step(&mut core, "fx_set_color2", |c| {
            c.fx_set_color2(0, Some(0xFF7BAC));
        });
        assert_single_undo_step(&mut core, "fx_set_anchor", |c| c.fx_set_anchor(0, 2, 4));
        assert_single_undo_step(&mut core, "fx_set_kind", |c| {
            c.fx_set_kind(0, ClipKind::ShockRing);
        });
        assert_single_undo_step(&mut core, "fx_set_hidden", |c| c.fx_set_hidden(0, true));
        assert_single_undo_step(&mut core, "fx_set_seed", |c| c.fx_set_seed(0, 99));
        assert_single_undo_step(&mut core, "fx_set_angle", |c| c.fx_set_angle(0, 45.0));
        assert_single_undo_step(&mut core, "fx_set_length", |c| c.fx_set_length(0, 200.0));
        // Every undo landed: the fixture is exactly the
        // clip it started as.
        assert_eq!(core.project.fx_clips.len(), 1);
        assert_eq!(core.project.fx_clips[0].clip.start, 4.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 10.0);
        assert_eq!(core.project.fx_clips[0].clip.kind, ClipKind::Burst);
        assert!(!core.project.fx_clips[0].hidden);
    }

    #[test]
    fn every_fx_op_refuses_on_an_empty_project_and_spends_nothing() {
        // The refusal half of the B4 contract, family-
        // wide: on a project with no FX clips, every op
        // refuses (status-only, `false` or `None` where
        // the op returns a verdict) and none of them
        // takes a snapshot — the project JSON and the
        // edit counter are both untouched after all thirteen.
        let mut core = core_with(vec![]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_move_clip(0, 1);
        core.fx_resize_clip(0, 1.0);
        assert!(!core.fx_split_clip(0, 0.5));
        assert_eq!(core.fx_duplicate_clip(0), None);
        core.fx_delete_clip(0);
        core.fx_set_color(0, 0xFFFFFF);
        core.fx_set_color2(0, Some(0xFFFFFF));
        core.fx_set_anchor(0, 0, 0);
        core.fx_set_kind(0, ClipKind::Wave);
        core.fx_set_hidden(0, true);
        core.fx_set_seed(0, 1);
        core.fx_set_angle(0, 45.0);
        core.fx_set_length(0, 200.0);
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(
            core.edits_since_autosave(),
            edits,
            "no refusal took a snapshot"
        );
        assert!(core.project.fx_clips.is_empty());
    }

    #[test]
    fn tool_set_hidden_sets_reads_back_and_is_undoable() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        // Wave 13 (B4): the handler path spends exactly
        // the op's one snapshot per successful call —
        // no extra snapshot at the handler layer.
        let edits = core.edits_since_autosave();
        let r = tool_fx_set_hidden(&mut core, &json!({ "index": 0, "hidden": true })).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["fx_index"], 0);
        assert_eq!(r["hidden"], true);
        assert_eq!(r["status"], "FX clip 1 hidden.");
        assert!(core.project.fx_clips[0].hidden);
        assert_eq!(core.edits_since_autosave(), edits + 1, "one snapshot via the handler");
        let r = tool_fx_set_hidden(&mut core, &json!({ "index": 0, "hidden": false })).unwrap();
        assert_eq!(r["hidden"], false);
        assert!(!core.project.fx_clips[0].hidden);
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per call");
    }

    #[test]
    fn tool_set_hidden_rejects_bad_input_without_panicking() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        // Wave 13 (B4): every rejection below must also
        // spend nothing on the edit counter (asserted at
        // the end of the test).
        let edits = core.edits_since_autosave();
        // Missing arguments.
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "hidden": true })).unwrap_err(),
            "missing required argument 'index'"
        );
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": 0 })).unwrap_err(),
            "missing required argument 'hidden'"
        );
        // Wrong types: a string flag, a fractional index, a
        // negative index, a null index (null = absent).
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": 0, "hidden": "yes" })).unwrap_err(),
            "argument 'hidden' must be a boolean"
        );
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": 0.5, "hidden": true })).unwrap_err(),
            "argument 'index' must be a non-negative integer"
        );
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": -1, "hidden": true })).unwrap_err(),
            "argument 'index' must be a non-negative integer"
        );
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": null, "hidden": true })).unwrap_err(),
            "missing required argument 'index'"
        );
        // Out-of-range indices, including an absurd one, are
        // the pre-check's clean error.
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": 1, "hidden": true })).unwrap_err(),
            "no FX clip at index 1"
        );
        assert_eq!(
            tool_fx_set_hidden(&mut core, &json!({ "index": 999999, "hidden": true })).unwrap_err(),
            "no FX clip at index 999999"
        );
        assert_eq!(project_json(&core), before, "no rejected call changed the project");
        assert_eq!(
            core.edits_since_autosave(),
            edits,
            "no rejected call took a snapshot"
        );
        assert!(!core.project.fx_clips[0].hidden);
    }

    #[test]
    fn set_seed_angle_length_write_and_snapshot_once() {
        // Wave 14 (A5): the three properties that were
        // readout-only land through their ops.
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_seed(0, 4242);
        assert_eq!(core.project.fx_clips[0].clip.seed, 4242);
        assert_eq!(core.status, "FX clip 1 seed set to 4242.");
        core.fx_set_angle(0, 45.0);
        assert_eq!(core.project.fx_clips[0].clip.angle, 45.0);
        assert_eq!(core.status, "FX clip 1 angle set to 45°.");
        core.fx_set_angle(0, -30.5);
        assert_eq!(core.project.fx_clips[0].clip.angle, -30.5);
        assert_eq!(core.status, "FX clip 1 angle set to -30.5°.");
        core.fx_set_length(0, 200.0);
        assert_eq!(core.project.fx_clips[0].clip.length, 200.0);
        assert_eq!(core.status, "FX clip 1 length set to 200.");
        assert_eq!(core.edits_since_autosave(), edits + 4, "one snapshot per op");
    }

    #[test]
    fn set_seed_angle_length_out_of_range_are_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_set_seed(9, 1);
        assert_eq!(core.status, "FX clip seed failed: no FX clip 10.");
        core.fx_set_angle(9, 45.0);
        assert_eq!(core.status, "FX clip angle failed: no FX clip 10.");
        core.fx_set_length(9, 200.0);
        assert_eq!(core.status, "FX clip length failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn tool_set_seed_angle_length_land_and_read_back() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Beam, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        let r = tool_fx_set_seed(&mut core, &json!({ "index": 0, "seed": 77 })).unwrap();
        assert_eq!(r["seed"], 77);
        assert_eq!(r["status"], "FX clip 1 seed set to 77.");
        assert_eq!(core.project.fx_clips[0].clip.seed, 77);
        let r = tool_fx_set_angle(&mut core, &json!({ "index": 0, "angle": 90.0 })).unwrap();
        assert_eq!(r["angle"], 90.0);
        assert_eq!(core.project.fx_clips[0].clip.angle, 90.0);
        let r = tool_fx_set_length(&mut core, &json!({ "index": 0, "length": 260.0 })).unwrap();
        assert_eq!(r["length"], 260.0);
        assert_eq!(core.project.fx_clips[0].clip.length, 260.0);
        assert_eq!(
            core.edits_since_autosave(),
            edits + 3,
            "exactly one snapshot per handler call"
        );
    }

    #[test]
    fn tool_set_seed_angle_length_reject_bad_input_without_spending() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        // Out-of-range index, in the file's pre-check voice.
        assert_eq!(
            tool_fx_set_seed(&mut core, &json!({ "index": 1, "seed": 1 })).unwrap_err(),
            "no FX clip at index 1"
        );
        // Missing and mistyped values.
        assert_eq!(
            tool_fx_set_seed(&mut core, &json!({ "index": 0 })).unwrap_err(),
            "missing required argument 'seed'"
        );
        assert_eq!(
            tool_fx_set_seed(&mut core, &json!({ "index": 0, "seed": -1 })).unwrap_err(),
            "argument 'seed' must be a non-negative integer"
        );
        assert_eq!(
            tool_fx_set_seed(&mut core, &json!({ "index": 0, "seed": u64::from(u32::MAX) + 1 }))
                .unwrap_err(),
            "argument 'seed' is too large"
        );
        assert_eq!(
            tool_fx_set_angle(&mut core, &json!({ "index": 0, "angle": "up" })).unwrap_err(),
            "argument 'angle' must be a number"
        );
        // A finite f64 that overflows the f32 cast is
        // refused by the finiteness contract.
        let huge = 1e300f64;
        assert_eq!(
            tool_fx_set_angle(&mut core, &json!({ "index": 0, "angle": huge })).unwrap_err(),
            "argument 'angle' must be a finite number"
        );
        assert_eq!(
            tool_fx_set_length(&mut core, &json!({ "index": 0, "length": huge })).unwrap_err(),
            "argument 'length' must be a finite number"
        );
        assert_eq!(project_json(&core), before, "no rejected call changed the project");
        assert_eq!(
            core.edits_since_autosave(),
            edits,
            "no rejected call took a snapshot"
        );
    }

    #[test]
    fn tool_preset_list_reports_an_array_and_its_count() {
        // The handler reads ./fx-presets/ relative to the
        // process working directory, whose contents the
        // test does not control — so this asserts the
        // reply's shape, not its contents: an ok flag, a
        // presets array, and a count that matches it.
        let mut core = core_with(vec![]);
        let r = tool_fx_preset_list(&mut core, &json!({})).unwrap();
        assert_eq!(r["ok"], true);
        let presets = r["presets"].as_array().expect("presets is an array");
        assert_eq!(r["count"], presets.len());
        assert!(presets.iter().all(|p| p.is_string()), "every preset is a slug string");
    }
}
