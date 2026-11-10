//! Wave P (P38) — MCP handlers for the one sprite
//! capability the audit proved this tree lacks:
//! addressing a sprite transform key by its EXACT
//! frame.
//!
//! The audit (Wave P, P38; findings in
//! `~/workspace/wave-p/findings/p38.md`) mapped the
//! whole sprite-instance surface —
//! `Project::sprites: Vec<SpriteInstance>` (an asset
//! index plus `InstanceKey`s `{ frame: f32, x, y,
//! scale, angle }`, kept sorted by frame) — across
//! the core ops (`src/sprite_ops.rs`), the MCP
//! handlers (`src/media.rs`), the Sprites panel
//! (`src/rune_sec/sprites_panel.rs`) and this tree's
//! dopesheet (`src/rune_sec/dopesheet.rs`):
//!
//! - List instances + keys; add an instance (stage
//!   centre, one frame-0 key, one undo step); remove
//!   an instance (one undo step); sample the
//!   interpolated transform; read the stored key at an
//!   exact frame (`sprite_key_at`, unclamped):
//!   COVERED (`sprite_list` / `sprite_add` /
//!   `sprite_remove` / `sprite_sample`, media.rs
//!   handlers over the core ops, mirrored by the
//!   panel).
//! - Write a key at a frame: COVERED, clamped —
//!   `set_sprite_key` clamps its frame argument to the
//!   project's last frame (the op family's
//!   `particle_ops` rule), and the clamp is honest:
//!   the core status and the MCP reply's stored key
//!   both name the frame actually written
//!   (report == effect; pinned at both layers).
//! - Delete a key: the clamped path was the hazard.
//!   `delete_sprite_key` clamped too, so a delete
//!   naming a frame at/past the frame count removed
//!   the LAST frame's key — and the MCP handler's
//!   success reply reported the REQUESTED frame. P38
//!   fixed the clamped path at both layers (the op
//!   refuses in its family voice; the handler refuses
//!   in the handler family's frame-range wording),
//!   spending nothing either way. The panel never hits
//!   the rule: it only ever names the current frame.
//! - THE GAP this module closes: a key left at/past
//!   the frame count when a timeline is shortened (a
//!   "stale" key). `sprite_list` reports it verbatim
//!   and `sprite_key_at` reads it exactly, but no tool
//!   could delete or retime it — every write path
//!   clamped or refused. This tree's dopesheet has NO
//!   sprite arms (it keys figures), so there is no GUI
//!   exact-frame sprite path to mirror; the two tools
//!   below therefore perform the write directly —
//!   the core retain/write under exactly ONE snapshot,
//!   in the sprite op family's own status voice:
//!   `sprite_move_key` retimes a key from an exact
//!   source frame (a stale source is the point) to a
//!   destination that must be inside the timeline (a
//!   past-the-end destination is REFUSED, never
//!   clamped), and `sprite_delete_key_at` deletes the
//!   key at exactly the frame named. The transform
//!   travels VERBATIM on a move — including a scale
//!   at or below zero, which an imported key may
//!   carry and which `sprite_set_key` refuses to
//!   write — and a key already sitting at the
//!   destination is replaced. A validated no-op
//!   (moving a key onto its own frame) spends no undo
//!   step; every real change is exactly one. Every
//!   write is verified from the resulting project
//!   state before success is reported.
//!
//! Argument helpers are LOCAL to this module with
//! `spt_`-prefixed names — `media`'s `ast_` helpers
//! are private to that module (the house precedent
//! for per-module MCP helper families). Each handler
//! carries its `// TOOL ENTRY SPEC:` (the registry
//! entry for `mcp_tools::tools()` plus its MCP.md
//! catalogue row) for the integrator to splice
//! verbatim; the module declaration in `lib.rs` is
//! likewise an integrator action (see the P38
//! findings).
//!
//! Wave X (X5) adds the SpriteLoop GAP P-3 pair beside
//! P38's — `sprite_set_asset` and `sprite_reorder`,
//! thin verified shells over the `sprite_ops` core ops
//! of the same names (see the section banner above
//! their handlers).

use crate::editor::EditorCore;
use crate::model::InstanceKey;
use serde_json::{json, Value};

// --------------------------------------------------------------------
// Argument helpers (local duplicates of the media.rs
// `ast_` shapes — see the module docs).
// --------------------------------------------------------------------

/// One argument of the MCP call (the `ast_get` shape,
/// media.rs).
fn spt_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.as_object().and_then(|o| o.get(key))
}

fn spt_as_usize(v: &Value, key: &str) -> Result<usize, String> {
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

/// Required non-negative integer argument, MCP
/// convention: a JSON `u64`, or an integral `f64` >= 0.
fn spt_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match spt_get(args, key) {
        Some(v) => spt_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Check a sprite instance index against the table
/// (the `ast_check_sprite` message shape, media.rs).
fn spt_check_sprite(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.sprites.len();
    if index >= n {
        return Err(format!(
            "sprite index {index} out of range (project has {n} sprite instance(s))"
        ));
    }
    Ok(())
}

/// Check an asset index against the table (the
/// `ast_check_asset` message shape, media.rs).
fn spt_check_asset(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.assets.len();
    if index >= n {
        return Err(format!(
            "asset index {index} out of range (project has {n} asset(s))"
        ));
    }
    Ok(())
}

/// One transform key as the wire object (the
/// `key_json` shape media.rs's sprite tools reply
/// with).
fn spt_key_json(key: &InstanceKey) -> Value {
    json!({
        "frame": key.frame,
        "x": key.x,
        "y": key.y,
        "scale": key.scale,
        "angle": key.angle,
    })
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 13 sprite block immediately after the
// sprite_delete_key entry (before the Wave 14
// parity-fill banner).
//   Tool {
//       name: "sprite_move_key",
//       description: "Move (retime) sprite 'sprite''s transform key from 'from_frame' to 'to_frame' by EXACT frame. The source frame is addressed exactly (never clamped), so a key left past the timeline end by a shortened timeline can be rescued back into range; the destination must be inside the timeline — a past-the-end destination is refused, never clamped. The stored transform travels verbatim (including a scale of 0 or less, which sprite_set_key refuses to write) and a key already at the destination is replaced. Moving a key onto its own frame changes nothing and spends no undo step; a real move is one undo step.",
//       schema: obj_schema(
//           json!({
//               "sprite": { "type": "integer" },
//               "from_frame": { "type": "integer" },
//               "to_frame": { "type": "integer" }
//           }),
//           &["sprite", "from_frame", "to_frame"],
//       ),
//       handler: crate::mcp_sprite_tools::tool_sprite_move_key,
//   }
// MCP.md row (insert after the `sprite_delete_key` row):
//   | `sprite_move_key` | Move (retime) sprite 'sprite''s transform key from 'from_frame' to 'to_frame' by exact frame. The source frame is addressed exactly (never clamped), so a key left past the timeline end by a shortened timeline can be rescued back into range; the destination must be inside the timeline — a past-the-end destination is refused, never clamped. The stored transform travels verbatim (including a scale of 0 or less, which `sprite_set_key` refuses to write) and a key already at the destination is replaced. Moving a key onto its own frame changes nothing and spends no undo step; a real move is one undo step. (Wave P, P38) | `sprite`, `from_frame`, `to_frame` | — |
/// MCP handler for `sprite_move_key`: retime sprite
/// `sprite`'s transform key from `from_frame` to
/// `to_frame`, addressing both frames exactly. There
/// is no core op for this write: the op family's
/// `set_sprite_key` clamps its frame argument (and
/// refuses a scale at or below zero), so the handler
/// performs the write itself under one snapshot, in
/// the op family's status voice. Validation order:
/// the source key must exist at the EXACT `from_frame`
/// (a stale key at/past the frame count is a legal
/// source — rescuing it is the point), then the
/// destination must be a frame inside the timeline
/// (refused past the end, never clamped), then
/// destination == source is a validated no-op that
/// spends no undo step. A real move is exactly one
/// snapshot; the apply removes the source key first,
/// then replaces a key sitting at the destination or
/// inserts at the sorted position, with the captured
/// key's transform verbatim. The result is verified
/// from the stored key list before success is
/// reported.
pub fn tool_sprite_move_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let sprite = spt_req_usize(args, "sprite")?;
    spt_check_sprite(core, sprite)?;
    let from = spt_req_usize(args, "from_frame")?;
    let to = spt_req_usize(args, "to_frame")?;
    // The plan, in its order: exact source lookup (no
    // clamp — a stale key may be the source), then the
    // destination range rule.
    let key = core.project.sprites[sprite]
        .keys
        .iter()
        .find(|k| k.frame == from as f32)
        .copied()
        .ok_or_else(|| {
            format!(
                "Sprite key move failed: sprite {} has no key at frame {}.",
                sprite + 1,
                from + 1
            )
        })?;
    if to >= core.frame_count() {
        return Err(format!(
            "Sprite key move failed: destination frame {} is past the last frame (project has {} frame(s)).",
            to + 1,
            core.frame_count()
        ));
    }
    if to == from {
        core.status = format!(
            "Sprite {} key is already at frame {}.",
            sprite + 1,
            from + 1
        );
        return Ok(json!({
            "ok": true,
            "sprite": sprite,
            "from_frame": from,
            "to_frame": to,
            "changed": false,
            "status": core.status,
        }));
    }
    core.snapshot_labeled("Move sprite key");
    let mut moved = key;
    moved.frame = to as f32;
    let keys = &mut core.project.sprites[sprite].keys;
    keys.retain(|k| k.frame != from as f32);
    match keys.iter().position(|k| k.frame == moved.frame) {
        Some(pos) => keys[pos] = moved,
        None => {
            let pos = keys.partition_point(|k| k.frame < moved.frame);
            keys.insert(pos, moved);
        }
    }
    core.status = format!(
        "Sprite {} key moved from frame {} to frame {}.",
        sprite + 1,
        from + 1,
        to + 1
    );
    // Verify the write from the resulting state: the
    // destination holds the captured key verbatim
    // (frame aside) and no key remains at the source
    // frame.
    let keys = &core.project.sprites[sprite].keys;
    let landed = keys.iter().find(|k| k.frame == to as f32);
    let source_gone = !keys.iter().any(|k| k.frame == from as f32);
    match landed {
        Some(k)
            if source_gone
                && k.x == key.x
                && k.y == key.y
                && k.scale == key.scale
                && k.angle == key.angle =>
        {
            Ok(json!({
                "ok": true,
                "sprite": sprite,
                "from_frame": from,
                "to_frame": to,
                "changed": true,
                "key": spt_key_json(k),
                "status": core.status,
            }))
        }
        _ => Err(core.status.clone()),
    }
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 13 sprite block immediately after the
// sprite_move_key entry.
//   Tool {
//       name: "sprite_delete_key_at",
//       description: "Delete sprite 'sprite''s transform key at EXACTLY 'frame': unlike sprite_delete_key, whose frame must be inside the timeline (a frame at/past the frame count is refused there, never clamped onto the last frame), this deletes the key at the frame named — including a stale key left past the timeline end by a shortened timeline — and never a different frame's key. A frame holding no key is refused before any change, so no undo step is spent. One undo step on success.",
//       schema: obj_schema(
//           json!({
//               "sprite": { "type": "integer" },
//               "frame": { "type": "integer" }
//           }),
//           &["sprite", "frame"],
//       ),
//       handler: crate::mcp_sprite_tools::tool_sprite_delete_key_at,
//   }
// MCP.md row (insert after the `sprite_move_key` row):
//   | `sprite_delete_key_at` | Delete sprite 'sprite''s transform key at exactly 'frame': unlike `sprite_delete_key`, whose frame must be inside the timeline (a frame at/past the frame count is refused there, never clamped onto the last frame), this deletes the key at the frame named — including a stale key left past the timeline end by a shortened timeline — and never a different frame's key. A frame holding no key is refused before any change, so no undo step is spent. One undo step on success. (Wave P, P38) | `sprite`, `frame` | — |
/// MCP handler for `sprite_delete_key_at`: delete
/// sprite `sprite`'s transform key at exactly
/// `frame`. There is no core op path for this delete:
/// the op family's `delete_sprite_key` validates its
/// frame against the timeline (Wave P, P38), so the
/// handler validates the exact frame itself and
/// retains under its own snapshot. For an in-range
/// frame its outcome is identical to
/// `sprite_delete_key`'s (same key removed, same
/// status wording — the core op's own sentence); the
/// tools differ only in the frame rule. Refusals (no
/// such sprite, or no key at exactly `frame`) are
/// raised before the snapshot, so they spend no undo
/// step; a real delete is exactly one.
pub fn tool_sprite_delete_key_at(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let sprite = spt_req_usize(args, "sprite")?;
    spt_check_sprite(core, sprite)?;
    let frame = spt_req_usize(args, "frame")?;
    if !core.project.sprites[sprite]
        .keys
        .iter()
        .any(|k| k.frame == frame as f32)
    {
        return Err(format!(
            "Sprite key delete failed: sprite {} has no key at frame {}.",
            sprite + 1,
            frame + 1
        ));
    }
    core.snapshot_labeled("Delete sprite key");
    core.project.sprites[sprite]
        .keys
        .retain(|k| k.frame != frame as f32);
    core.status = format!(
        "Sprite {} key at frame {} deleted.",
        sprite + 1,
        frame + 1
    );
    if core.project.sprites[sprite]
        .keys
        .iter()
        .any(|k| k.frame == frame as f32)
    {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "sprite": sprite,
        "frame": frame,
        "key_count": core.project.sprites[sprite].keys.len(),
        "status": core.status,
    }))
}

// --------------------------------------------------------------------
// Wave X (X5) — SpriteLoop GAP P-3's two instance-level
// writes: re-pointing an instance at another asset, and
// z-reorder. Both drive the `sprite_ops` core ops
// (`set_sprite_asset` / `reorder_sprite`), which own the
// mutation, the status line and exactly one snapshot per
// real change; the handlers validate arguments up front,
// then verify the op's effect from the resulting project
// state — the media.rs sprite handlers' discipline, in
// this module because the registry splice rides the same
// TOOL ENTRY SPEC route as P38's pair above.
// --------------------------------------------------------------------

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 13 sprite block immediately after the
// sprite_delete_key_at entry.
//   Tool {
//       name: "sprite_set_asset",
//       description: "Re-point sprite 'sprite' at image asset 'asset', keeping every transform key verbatim — the fix for a wrong image, which otherwise means removing the instance and re-keying everything. Naming the asset the sprite already uses changes nothing and spends no undo step. One undo step on a real change.",
//       schema: obj_schema(
//           json!({
//               "sprite": { "type": "integer" },
//               "asset": { "type": "integer" }
//           }),
//           &["sprite", "asset"],
//       ),
//       handler: crate::mcp_sprite_tools::tool_sprite_set_asset,
//   }
// MCP.md row (insert after the `sprite_delete_key_at` row):
//   | `sprite_set_asset` | Re-point sprite 'sprite' at image asset 'asset', keeping every transform key verbatim — the fix for a wrong image, which otherwise means removing the instance and re-keying everything. Naming the asset the sprite already uses changes nothing and spends no undo step. One undo step on a real change. (Wave X, X5) | `sprite`, `asset` | — |
/// MCP handler for `sprite_set_asset`: re-point sprite
/// `sprite` at asset `asset` via the `set_sprite_asset`
/// core op. The op's validated no-op (the asset already
/// in use) reports `changed: false` and spends no undo
/// step; a real change is exactly one. The result is
/// verified from the resulting state — the instance's
/// asset is the one named and its key list is
/// byte-for-byte the one captured before the call —
/// before success is reported.
pub fn tool_sprite_set_asset(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let sprite = spt_req_usize(args, "sprite")?;
    spt_check_sprite(core, sprite)?;
    let asset = spt_req_usize(args, "asset")?;
    spt_check_asset(core, asset)?;
    let keys_before: Vec<Value> = core.project.sprites[sprite]
        .keys
        .iter()
        .map(spt_key_json)
        .collect();
    let changed = core.project.sprites[sprite].asset != asset;
    core.set_sprite_asset(sprite, asset);
    let inst = &core.project.sprites[sprite];
    let keys_after: Vec<Value> = inst.keys.iter().map(spt_key_json).collect();
    if inst.asset == asset && keys_after == keys_before {
        Ok(json!({
            "ok": true,
            "sprite": sprite,
            "asset": asset,
            "changed": changed,
            "key_count": inst.keys.len(),
            "status": core.status,
        }))
    } else {
        Err(core.status.clone())
    }
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 13 sprite block immediately after the
// sprite_set_asset entry.
//   Tool {
//       name: "sprite_reorder",
//       description: "Move sprite 'sprite' to 0-based position 'to' in the sprite table — draw order IS table order (later sprites paint over earlier ones), so this is the z-order write. The instances between shift by one, keeping their relative order; the moved sprite keeps its asset and keys verbatim. Naming the position the sprite already holds changes nothing and spends no undo step. One undo step on a real move.",
//       schema: obj_schema(
//           json!({
//               "sprite": { "type": "integer" },
//               "to": { "type": "integer" }
//           }),
//           &["sprite", "to"],
//       ),
//       handler: crate::mcp_sprite_tools::tool_sprite_reorder,
//   }
// MCP.md row (insert after the `sprite_set_asset` row):
//   | `sprite_reorder` | Move sprite 'sprite' to 0-based position 'to' in the sprite table — draw order IS table order (later sprites paint over earlier ones), so this is the z-order write. The instances between shift by one, keeping their relative order; the moved sprite keeps its asset and keys verbatim. Naming the position the sprite already holds changes nothing and spends no undo step. One undo step on a real move. (Wave X, X5) | `sprite`, `to` | — |
/// MCP handler for `sprite_reorder`: move sprite
/// `sprite` to position `to` via the `reorder_sprite`
/// core op. A `to` outside the table is refused up
/// front (the handler family's index wording) before
/// any change; the op's validated no-op (the position
/// already held) reports `changed: false` and spends
/// no undo step. The result is verified from the
/// resulting table — the instance at `to` is the one
/// captured before the call (asset + keys verbatim)
/// and the table length is unchanged — before success
/// is reported.
pub fn tool_sprite_reorder(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let sprite = spt_req_usize(args, "sprite")?;
    spt_check_sprite(core, sprite)?;
    let to = spt_req_usize(args, "to")?;
    let n = core.project.sprites.len();
    if to >= n {
        return Err(format!(
            "position index {to} out of range (project has {n} sprite instance(s))"
        ));
    }
    let before = core.project.sprites[sprite].clone();
    let keys_before: Vec<Value> = before.keys.iter().map(spt_key_json).collect();
    core.reorder_sprite(sprite, to);
    let landed = &core.project.sprites[to];
    let keys_after: Vec<Value> = landed.keys.iter().map(spt_key_json).collect();
    if core.project.sprites.len() == n
        && landed.asset == before.asset
        && keys_after == keys_before
    {
        Ok(json!({
            "ok": true,
            "sprite": sprite,
            "from": sprite,
            "to": to,
            "changed": sprite != to,
            "status": core.status,
        }))
    } else {
        Err(core.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ImageAsset, SpriteInstance};

    /// A demo-project core (5 frames) whose asset table
    /// holds three dummy entries (names only — the
    /// handlers never decode bytes) and whose sprite
    /// table holds the given instances, set up by direct
    /// field writes so the undo depth starts at exactly
    /// 0 (the `sprite_ops` fixture pattern).
    fn core_with_sprites(sprites: Vec<SpriteInstance>) -> EditorCore {
        let mut core = EditorCore::new();
        for i in 0..3 {
            core.project.assets.push(ImageAsset {
                name: format!("asset-{i}.png"),
                png: Vec::new(),
            });
        }
        core.project.sprites = sprites;
        core
    }

    fn key(frame: f32, x: f32, y: f32, scale: f32, angle: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x,
            y,
            scale,
            angle,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    fn sprite(asset: usize, keys: Vec<InstanceKey>) -> SpriteInstance {
        SpriteInstance { asset, keys }
    }

    /// The whole project as its snapshot JSON — the
    /// exact bytes `snapshot()` would store. `Project`
    /// has no `PartialEq`, so this is how "untouched" is
    /// asserted (the `sprite_ops` pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    fn frames_of(core: &EditorCore, sprite: usize) -> Vec<f32> {
        core.project.sprites[sprite]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect()
    }

    // ---- the gap: stale keys ----

    #[test]
    fn stale_key_is_rescued_into_range_in_one_undo_step() {
        // Frame 8 in a 5-frame project: at/past the frame
        // count — the stale shape a shortened timeline
        // leaves behind.
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(1.0, 100.0, 100.0, 1.0, 0.0),
                key(8.0, 640.0, 480.0, 2.0, 30.0),
            ],
        )]);
        let out = tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 8, "to_frame": 3 }),
        )
        .expect("a stale key may be moved back into the timeline");
        assert_eq!(out["changed"], json!(true));
        assert_eq!(out["status"], json!("Sprite 1 key moved from frame 9 to frame 4."));
        assert_eq!(frames_of(&core, 0), vec![1.0, 3.0], "sorted, stale frame gone");
        let landed = core.project.sprites[0].keys[1];
        assert_eq!(
            (landed.x, landed.y, landed.scale, landed.angle),
            (640.0, 480.0, 2.0, 30.0),
            "the transform travels verbatim"
        );
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
        core.undo();
        assert_eq!(frames_of(&core, 0), vec![1.0, 8.0], "one undo restores the stale key");
    }

    #[test]
    fn stale_key_is_deleted_at_its_exact_frame() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(4.0, 10.0, 10.0, 1.0, 0.0),
                key(8.0, 640.0, 480.0, 2.0, 30.0),
            ],
        )]);
        let out = tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 0, "frame": 8 }))
            .expect("a stale key may be deleted at its exact frame");
        assert_eq!(out["key_count"], json!(1));
        assert_eq!(out["status"], json!("Sprite 1 key at frame 9 deleted."));
        assert_eq!(
            frames_of(&core, 0),
            vec![4.0],
            "frame 4's key is untouched — the clamped tool would have acted on it instead"
        );
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
        core.undo();
        assert_eq!(frames_of(&core, 0), vec![4.0, 8.0], "one undo restores the stale key");
    }

    // ---- the in-range move ----

    #[test]
    fn move_replaces_the_key_at_the_destination() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(1.0, 100.0, 100.0, 1.0, 0.0),
                key(3.0, 300.0, 300.0, 1.5, 45.0),
            ],
        )]);
        tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 1, "to_frame": 3 }),
        )
        .expect("the move succeeds");
        assert_eq!(
            frames_of(&core, 0),
            vec![3.0],
            "the destination key is replaced, not duplicated"
        );
        let landed = core.project.sprites[0].keys[0];
        assert_eq!(
            (landed.x, landed.y, landed.scale, landed.angle),
            (100.0, 100.0, 1.0, 0.0),
            "the moved key's values, not the replaced key's"
        );
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
    }

    #[test]
    fn move_inserts_at_the_sorted_position() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(0.0, 0.0, 0.0, 1.0, 0.0),
                key(4.0, 90.0, 90.0, 1.0, 0.0),
            ],
        )]);
        tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 0, "to_frame": 2 }),
        )
        .expect("the move succeeds");
        assert_eq!(frames_of(&core, 0), vec![2.0, 4.0], "the key list stays sorted");
    }

    #[test]
    fn move_carries_a_zero_scale_key_verbatim() {
        // An imported key may carry scale <= 0 (stored by
        // the op family, skipped by the compositor);
        // `sprite_set_key` refuses to write such a key,
        // so a delete+set composition could never retime
        // it — the verbatim move can.
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![key(2.0, 50.0, 60.0, 0.0, -15.0)],
        )]);
        let out = tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 2, "to_frame": 3 }),
        )
        .expect("the verbatim move succeeds");
        assert_eq!(out["key"]["scale"], json!(0.0));
        assert_eq!(core.project.sprites[0].keys[0].scale, 0.0);
        assert_eq!(core.project.sprites[0].keys[0].angle, -15.0);
    }

    #[test]
    fn move_onto_its_own_frame_is_a_no_op() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![key(2.0, 50.0, 60.0, 1.0, 0.0)],
        )]);
        let before = project_json(&core);
        let out = tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 2, "to_frame": 2 }),
        )
        .expect("a same-frame move is a validated no-op");
        assert_eq!(out["changed"], json!(false));
        assert_eq!(out["status"], json!("Sprite 1 key is already at frame 3."));
        assert_eq!(project_json(&core), before, "nothing was written");
        assert_eq!(core.undo_depth(), 0, "a no-op spends no undo step");
    }

    // ---- refusals spend nothing ----

    #[test]
    fn move_refusals_spend_no_undo_step() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![key(8.0, 640.0, 480.0, 2.0, 30.0)],
        )]);
        let before = project_json(&core);
        // No such sprite (the MCP house index check).
        assert!(tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 1, "from_frame": 8, "to_frame": 0 })
        )
        .is_err());
        // No key at the named source frame.
        let err = tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 7, "to_frame": 0 }),
        )
        .expect_err("frame 7 holds no key");
        assert_eq!(err, "Sprite key move failed: sprite 1 has no key at frame 8.");
        // A destination at/past the frame count is
        // refused, never clamped to the last frame.
        let err = tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 8, "to_frame": 5 }),
        )
        .expect_err("frame 5 is the first frame past the end");
        assert_eq!(
            err,
            "Sprite key move failed: destination frame 6 is past the last frame (project has 5 frame(s))."
        );
        // The stale frame as its own destination hits
        // the range rule before the no-op rule (the
        // validation order).
        assert!(tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": 8, "to_frame": 8 })
        )
        .is_err());
        // Missing and mistyped arguments.
        assert!(tool_sprite_move_key(&mut core, &json!({ "sprite": 0, "from_frame": 8 })).is_err());
        assert!(tool_sprite_move_key(
            &mut core,
            &json!({ "sprite": 0, "from_frame": -1, "to_frame": 0 })
        )
        .is_err());
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.undo_depth(), 0, "refusals spend no undo step");
    }

    #[test]
    fn delete_at_refusals_spend_no_undo_step() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![key(8.0, 640.0, 480.0, 2.0, 30.0)],
        )]);
        let before = project_json(&core);
        // No such sprite (the MCP house index check).
        assert!(tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 3, "frame": 8 })).is_err());
        // No key at exactly the named frame — frame 9 is
        // not licensed by the stale key at frame 8.
        let err = tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 0, "frame": 9 }))
            .expect_err("frame 9 holds no key");
        assert_eq!(err, "Sprite key delete failed: sprite 1 has no key at frame 10.");
        // An in-range frame with no key refuses the same
        // way (exactness cuts both ways).
        assert!(tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 0, "frame": 0 })).is_err());
        // Missing argument.
        assert!(tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 0 })).is_err());
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.undo_depth(), 0, "refusals spend no undo step");
    }

    // ---- Wave X (X5): set_asset + reorder ----

    #[test]
    fn set_asset_repoints_and_keeps_the_keys_verbatim() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(0.0, 100.0, 100.0, 1.0, 0.0),
                key(4.0, 300.0, 200.0, 3.0, 90.0),
            ],
        )]);
        let out = tool_sprite_set_asset(&mut core, &json!({ "sprite": 0, "asset": 2 }))
            .expect("the re-point succeeds");
        assert_eq!(out["changed"], json!(true));
        assert_eq!(out["key_count"], json!(2));
        assert_eq!(out["status"], json!("Sprite 1 now uses asset 3."));
        assert_eq!(core.project.sprites[0].asset, 2);
        assert_eq!(frames_of(&core, 0), vec![0.0, 4.0], "no key moved");
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
        core.undo();
        assert_eq!(core.project.sprites[0].asset, 0, "one undo restores the asset");
    }

    #[test]
    fn set_asset_same_asset_is_a_no_op() {
        let mut core = core_with_sprites(vec![sprite(1, vec![key(0.0, 5.0, 5.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        let out = tool_sprite_set_asset(&mut core, &json!({ "sprite": 0, "asset": 1 }))
            .expect("naming the current asset is a validated no-op");
        assert_eq!(out["changed"], json!(false));
        assert_eq!(out["status"], json!("Sprite 1 already uses asset 2."));
        assert_eq!(project_json(&core), before, "nothing was written");
        assert_eq!(core.undo_depth(), 0, "a no-op spends no undo step");
    }

    #[test]
    fn set_asset_refusals_spend_no_undo_step() {
        let mut core = core_with_sprites(vec![sprite(0, vec![key(0.0, 5.0, 5.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        // No such sprite / no such asset (the MCP house
        // index checks, media.rs shapes).
        let err = tool_sprite_set_asset(&mut core, &json!({ "sprite": 1, "asset": 0 }))
            .expect_err("no sprite 1");
        assert_eq!(err, "sprite index 1 out of range (project has 1 sprite instance(s))");
        let err = tool_sprite_set_asset(&mut core, &json!({ "sprite": 0, "asset": 3 }))
            .expect_err("the fixture holds three assets");
        assert_eq!(err, "asset index 3 out of range (project has 3 asset(s))");
        // Missing argument.
        assert!(tool_sprite_set_asset(&mut core, &json!({ "sprite": 0 })).is_err());
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.undo_depth(), 0, "refusals spend no undo step");
    }

    #[test]
    fn reorder_moves_the_instance_to_the_named_position() {
        let mut core = core_with_sprites(vec![
            sprite(0, vec![key(0.0, 10.0, 0.0, 1.0, 0.0)]),
            sprite(1, vec![key(0.0, 20.0, 0.0, 1.0, 0.0)]),
            sprite(2, vec![key(0.0, 30.0, 0.0, 1.0, 0.0)]),
        ]);
        let out = tool_sprite_reorder(&mut core, &json!({ "sprite": 0, "to": 2 }))
            .expect("the move succeeds");
        assert_eq!(out["changed"], json!(true));
        assert_eq!(out["status"], json!("Sprite moved from position 1 to position 3."));
        let assets: Vec<usize> = core.project.sprites.iter().map(|s| s.asset).collect();
        assert_eq!(assets, vec![1, 2, 0], "the rest keep their relative order");
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
        core.undo();
        let assets: Vec<usize> = core.project.sprites.iter().map(|s| s.asset).collect();
        assert_eq!(assets, vec![0, 1, 2], "one undo restores the order");
    }

    #[test]
    fn reorder_onto_its_own_position_is_a_no_op() {
        let mut core = core_with_sprites(vec![
            sprite(0, vec![key(0.0, 10.0, 0.0, 1.0, 0.0)]),
            sprite(1, vec![key(0.0, 20.0, 0.0, 1.0, 0.0)]),
        ]);
        let before = project_json(&core);
        let out = tool_sprite_reorder(&mut core, &json!({ "sprite": 1, "to": 1 }))
            .expect("naming the held position is a validated no-op");
        assert_eq!(out["changed"], json!(false));
        assert_eq!(out["status"], json!("Sprite 2 is already at position 2."));
        assert_eq!(project_json(&core), before, "nothing was written");
        assert_eq!(core.undo_depth(), 0, "a no-op spends no undo step");
    }

    #[test]
    fn reorder_refusals_spend_no_undo_step() {
        let mut core = core_with_sprites(vec![
            sprite(0, vec![key(0.0, 10.0, 0.0, 1.0, 0.0)]),
            sprite(1, vec![key(0.0, 20.0, 0.0, 1.0, 0.0)]),
        ]);
        let before = project_json(&core);
        assert!(tool_sprite_reorder(&mut core, &json!({ "sprite": 2, "to": 0 })).is_err());
        let err = tool_sprite_reorder(&mut core, &json!({ "sprite": 0, "to": 2 }))
            .expect_err("position 2 is past the two-instance table");
        assert_eq!(err, "position index 2 out of range (project has 2 sprite instance(s))");
        assert!(tool_sprite_reorder(&mut core, &json!({ "sprite": 0 })).is_err());
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.undo_depth(), 0, "refusals spend no undo step");
    }

    #[test]
    fn delete_at_in_range_matches_the_op_outcome() {
        let mut core = core_with_sprites(vec![sprite(
            0,
            vec![
                key(0.0, 0.0, 0.0, 1.0, 0.0),
                key(2.0, 50.0, 50.0, 1.0, 0.0),
                key(4.0, 90.0, 90.0, 1.0, 0.0),
            ],
        )]);
        let out = tool_sprite_delete_key_at(&mut core, &json!({ "sprite": 0, "frame": 2 }))
            .expect("the delete succeeds");
        assert_eq!(out["status"], json!("Sprite 1 key at frame 3 deleted."));
        assert_eq!(frames_of(&core, 0), vec![0.0, 4.0], "only that key is removed");
        assert_eq!(core.undo_depth(), 1, "exactly one undo step");
    }
}
