//! Sprite-instance engine ops: the `EditorCore` method
//! family for editing the project's free-floating sprite
//! instances (`Project::sprites`) — add, remove, and
//! keyframe their transforms (Wave 13, squad A, item A1).
//!
//! Before this module a `SpriteInstance` could only enter
//! a project through Stick Nodes import (`nodes_anim.rs`)
//! — the rasterizer, the SVG exporter and the stage's
//! placeholder outlines all READ instances
//! (`media::media_items` composites them after the
//! segment media), but no op could create, delete or
//! re-key one, so the shell had no sprite surface at all.
//! These ops are the engine home the Sprites panel
//! (squad A3) and the MCP sprite handlers (squad A4)
//! call, written in the `particle_ops.rs` / `fx_ops.rs`
//! house style:
//!
//! * An out-of-range sprite or asset index changes
//!   nothing and takes NO snapshot — it only sets
//!   `status`, in the failure voice of the neighbouring
//!   ops ("… failed: no sprite N.").
//! * An in-range mutating op snapshots exactly once
//!   (one undo step), then writes the field directly
//!   (there is no backend module for sprites the way
//!   `fx_edit` backs `fx_ops`).
//! * Success is reported on `status` with 1-based sprite,
//!   asset and frame numbers, like `particle_add_event`.
//!
//! The model (see `model.rs`): a `SpriteInstance` is an
//! asset index plus a list of `InstanceKey`s
//! (`frame: f32, x, y, scale, angle` — the angle in
//! DEGREES, y-down like the rest of the model:
//! `media_items` feeds it to `to_radians` for the quad
//! and passes it through as a text angle). Sampling is
//! `InstanceKey::sample`: linear interpolation between
//! the surrounding keys, clamped to the first/last key
//! outside the keyed range, `None` for an empty key
//! list. That sampler walks the list in stored order
//! and reads `keys[0]` as the earliest key, so the ops
//! here keep every key list sorted by frame:
//! `set_sprite_key` replaces a key written at the same
//! frame and inserts a new one at its sorted position.
//!
//! Deliberate value rules, each matching a neighbouring
//! contract rather than inventing a new one:
//!
//! * `add_sprite` places the instance at the stage
//!   centre (`project.stage / 2`) with ONE key at frame
//!   0, scale 1.0, angle 0.0 — the brief for this module,
//!   and the same single-key birth `text_edit`'s
//!   `add_text_instance` gives a text instance. The
//!   asset index is bounds-checked only: the asset
//!   table holds validated PNGs by construction (the
//!   import path in `media.rs` checks magic bytes and
//!   decodability), and the renderers skip an asset
//!   whose bytes do not decode, so the op does not
//!   decode anything.
//! * Frame arguments are clamped to the project's last
//!   frame — the `particle_ops` rule: a key placed past
//!   the end could never be reached by the playhead. The
//!   one exception is `delete_sprite_key` (Wave P, P38):
//!   it REFUSES a frame at/past the frame count instead
//!   of clamping, because a clamped delete would remove
//!   the LAST frame's key while the caller named a
//!   different frame — a write may land somewhere safe,
//!   a delete must land exactly where it was aimed or
//!   not at all. A key left past the end by a shortened
//!   timeline (a "stale" key) is deleted or retimed by
//!   exact frame through the MCP tools
//!   `sprite_delete_key_at` / `sprite_move_key`
//!   (src/mcp_sprite_tools.rs), which address frames
//!   with no clamp at all.
//! * Non-finite transform values (x / y / scale / angle)
//!   are a status-only refusal, the `set_project_fps`
//!   discipline in `editor.rs`. Nothing else is clamped:
//!   a sprite may sit off-stage (the camera moves), an
//!   angle is stored unwrapped (the sampler lerps the
//!   raw values, so wrapping would change the
//!   interpolation), and a scale at or below zero is
//!   stored but renders nothing — that invisibility is
//!   `media_items`' own rule, named here so nobody
//!   mistakes it for an op-side clamp.
//! * Deleting an instance's LAST key is allowed: the
//!   instance stays in the table with an empty key list,
//!   samples to `None` and renders nothing (the sampler
//!   and `media_items` both already tolerate it).
//!   `remove_sprite` is the op that deletes the
//!   instance itself.
//! * Wave X (X5, SpriteLoop GAP P-3) adds the two
//!   instance-level writes the small-wins pack named:
//!   `set_sprite_asset` re-points an instance at
//!   another asset WITHOUT touching its keys (the
//!   delete + re-key workaround was the friction), and
//!   `reorder_sprite` moves an instance to a new
//!   position in the table — draw order IS vec order
//!   (`media_items` composites in table order, the
//!   stage paints in table order), so position is the
//!   z-order and no other field needs to exist. Both
//!   follow the family's rules above, plus the
//!   validated-no-op rule `sprite_move_key` set in
//!   Wave P: naming the asset/position the instance
//!   already has changes nothing and spends no
//!   snapshot.
//!
//! Integrator wiring: declare `pub mod sprite_ops;` in
//! `lib.rs` (alphabetical: after `pub mod shell;`,
//! before `pub mod stats;`). Nothing else is needed —
//! this file only adds an inherent `impl EditorCore`
//! block.

use crate::editor::EditorCore;
use crate::model::{InstanceKey, SpriteInstance};

impl EditorCore {
    /// Clamp a frame argument to the project's last frame
    /// (0 for a project with no frames — defensive; a real
    /// project always has at least one). The
    /// `particle_ops` clamp, used by `set_sprite_key`;
    /// `delete_sprite_key` validates its frame instead
    /// (see the module docs).
    fn sprite_clamp_frame(&self, frame: usize) -> usize {
        frame.min(self.frame_count().saturating_sub(1))
    }

    /// Add a sprite instance for asset `asset`: one key
    /// at frame 0 placing it at the stage centre, scale
    /// 1.0, angle 0.0 (see the module docs). One undo
    /// step. Returns the new instance's index, so the
    /// caller can select what it just made; an
    /// out-of-range asset index only sets the status and
    /// returns `None`.
    pub fn add_sprite(&mut self, asset: usize) -> Option<usize> {
        if self.project.assets.get(asset).is_none() {
            self.status = format!("Sprite add failed: no asset {}.", asset + 1);
            return None;
        }
        let centre = [self.project.stage[0] / 2.0, self.project.stage[1] / 2.0];
        self.snapshot_labeled("Add sprite");
        self.project.sprites.push(SpriteInstance {
            asset,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: centre[0],
                y: centre[1],
                scale: 1.0,
                angle: 0.0,
                // The P-6 enrichment at its defaults: opaque,
                // unflipped, centred pivot, scale_y following
                // scale (Wave X, X9).
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let index = self.project.sprites.len() - 1;
        self.status = format!("Sprite {} added (asset {}).", index + 1, asset + 1);
        Some(index)
    }

    /// Delete sprite instance `index` (the whole
    /// instance, keys and all). One undo step; an
    /// out-of-range index only sets the status.
    pub fn remove_sprite(&mut self, index: usize) {
        if self.project.sprites.get(index).is_none() {
            self.status = format!("Sprite delete failed: no sprite {}.", index + 1);
            return;
        }
        self.snapshot_labeled("Delete sprite");
        self.project.sprites.remove(index);
        self.status = format!("Sprite {} deleted.", index + 1);
    }

    /// Write sprite `index`'s transform key at `frame`:
    /// position `(x, y)`, `scale`, and `angle` in degrees
    /// (the model's field name and unit). If the instance
    /// already has a key at that frame it is replaced;
    /// otherwise the key is inserted at its sorted
    /// position, so the key list stays in the frame order
    /// `InstanceKey::sample` assumes. `frame` is clamped
    /// to the project's last frame. One undo step;
    /// refusals (no such sprite, or a non-finite value)
    /// only set the status.
    ///
    /// The P-6 enrichment fields are NOT this op's to
    /// write: a replaced key keeps its opacity, flips,
    /// scale_y and pivot (a panel stepper press must not
    /// wipe an imported sprite's flips), and a fresh key
    /// takes their defaults. `set_sprite_key_full` is the
    /// write that sets them.
    pub fn set_sprite_key(
        &mut self,
        index: usize,
        frame: usize,
        x: f32,
        y: f32,
        scale: f32,
        angle: f32,
    ) {
        if self.project.sprites.get(index).is_none() {
            self.status = format!("Sprite key failed: no sprite {}.", index + 1);
            return;
        }
        if ![x, y, scale, angle].iter().all(|v| v.is_finite()) {
            self.status = "Sprite key failed: values must be finite.".into();
            return;
        }
        let frame = self.sprite_clamp_frame(frame);
        let prev = self.project.sprites[index]
            .keys
            .iter()
            .find(|k| k.frame == frame as f32);
        let key = InstanceKey {
            frame: frame as f32,
            x,
            y,
            scale,
            angle,
            opacity: prev.map_or(1.0, |k| k.opacity),
            flip_x: prev.is_some_and(|k| k.flip_x),
            flip_y: prev.is_some_and(|k| k.flip_y),
            scale_y: prev.map_or(0.0, |k| k.scale_y),
            pivot_x: prev.map_or(0.5, |k| k.pivot_x),
            pivot_y: prev.map_or(0.5, |k| k.pivot_y),
        };
        self.write_sprite_key(index, key);
    }

    /// Write a COMPLETE key (all eleven fields, Wave X P-6)
    /// for sprite `index` at `frame` — the enrichment write
    /// behind the MCP `sprite_set_key` handler's optional
    /// opacity / flip / scale_y / pivot arguments. The
    /// stored frame is `frame` (clamped to the project's
    /// last frame, the family's rule); `key.frame` is
    /// ignored in favour of it. Replace/insert, the status
    /// and the one-undo-step discipline are
    /// `set_sprite_key`'s own; refusals (no such sprite, or
    /// a non-finite value in any numeric field) only set
    /// the status.
    pub fn set_sprite_key_full(&mut self, index: usize, frame: usize, key: InstanceKey) {
        if self.project.sprites.get(index).is_none() {
            self.status = format!("Sprite key failed: no sprite {}.", index + 1);
            return;
        }
        let values = [
            key.x,
            key.y,
            key.scale,
            key.angle,
            key.opacity,
            key.scale_y,
            key.pivot_x,
            key.pivot_y,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            self.status = "Sprite key failed: values must be finite.".into();
            return;
        }
        let frame = self.sprite_clamp_frame(frame);
        let key = InstanceKey {
            frame: frame as f32,
            ..key
        };
        self.write_sprite_key(index, key);
    }

    /// The shared tail of the two key writes: snapshot
    /// once, replace the key at `key.frame` or insert it at
    /// its sorted position, and set the success status.
    fn write_sprite_key(&mut self, index: usize, key: InstanceKey) {
        self.snapshot_labeled("Set sprite key");
        let keys = &mut self.project.sprites[index].keys;
        match keys.iter().position(|k| k.frame == key.frame) {
            Some(pos) => keys[pos] = key,
            None => {
                let pos = keys.partition_point(|k| k.frame < key.frame);
                keys.insert(pos, key);
            }
        }
        self.status = format!(
            "Sprite {} key set at frame {}.",
            index + 1,
            key.frame as usize + 1
        );
    }

    /// Delete sprite `index`'s key at `frame`. Unlike the
    /// write path, the frame is NOT clamped: a frame
    /// at/past the project's frame count is refused
    /// outright (Wave P, P38 — a clamped delete removes
    /// the last frame's key under a past-end request; see
    /// the module docs). Deleting the last remaining key
    /// is allowed and leaves an instance that renders
    /// nothing — see the module docs. One undo step;
    /// refusals (no such sprite, a frame past the end,
    /// or no key at that frame) only set the status.
    pub fn delete_sprite_key(&mut self, index: usize, frame: usize) {
        if self.project.sprites.get(index).is_none() {
            self.status = format!(
                "Sprite key delete failed: no sprite {}.",
                index + 1
            );
            return;
        }
        if frame >= self.frame_count() {
            self.status = format!(
                "Sprite key delete failed: frame {} is past the last frame (project has {} frame(s)).",
                frame + 1,
                self.frame_count()
            );
            return;
        }
        let frame_f = frame as f32;
        if !self.project.sprites[index]
            .keys
            .iter()
            .any(|k| k.frame == frame_f)
        {
            self.status = format!(
                "Sprite key delete failed: sprite {} has no key at frame {}.",
                index + 1,
                frame + 1
            );
            return;
        }
        self.snapshot_labeled("Delete sprite key");
        self.project.sprites[index]
            .keys
            .retain(|k| k.frame != frame_f);
        self.status = format!(
            "Sprite {} key at frame {} deleted.",
            index + 1,
            frame + 1
        );
    }

    /// Re-point sprite `index` at asset `asset`,
    /// keeping every one of its transform keys verbatim
    /// (Wave X, X5 — SpriteLoop GAP P-3: a wrong image
    /// used to mean delete the instance and re-key
    /// everything). Only the `asset` field changes; the
    /// instance's position in the table (its z-order)
    /// is likewise untouched. One undo step; refusals
    /// (no such sprite, no such asset) only set the
    /// status, and naming the asset the instance
    /// already uses is a validated no-op that spends
    /// no snapshot (the `sprite_move_key` discipline).
    pub fn set_sprite_asset(&mut self, index: usize, asset: usize) {
        if self.project.sprites.get(index).is_none() {
            self.status = format!("Sprite asset failed: no sprite {}.", index + 1);
            return;
        }
        if self.project.assets.get(asset).is_none() {
            self.status = format!("Sprite asset failed: no asset {}.", asset + 1);
            return;
        }
        if self.project.sprites[index].asset == asset {
            self.status = format!(
                "Sprite {} already uses asset {}.",
                index + 1,
                asset + 1
            );
            return;
        }
        self.snapshot_labeled("Set sprite asset");
        self.project.sprites[index].asset = asset;
        self.status = format!(
            "Sprite {} now uses asset {}.",
            index + 1,
            asset + 1
        );
    }

    /// Move sprite `index` to position `to` in the
    /// instance table (Wave X, X5 — SpriteLoop GAP
    /// P-3's z-reorder: draw order is vec order, so
    /// this IS the z-order write). The instances
    /// between the two positions shift by one, keeping
    /// their relative order; the moved instance keeps
    /// its asset and keys verbatim. One undo step;
    /// refusals (no such sprite, no such position)
    /// only set the status, and naming the position
    /// the instance already holds is a validated no-op
    /// that spends no snapshot.
    pub fn reorder_sprite(&mut self, index: usize, to: usize) {
        let n = self.project.sprites.len();
        if index >= n {
            self.status = format!("Sprite reorder failed: no sprite {}.", index + 1);
            return;
        }
        if to >= n {
            self.status = format!(
                "Sprite reorder failed: no position {} (project has {} sprite instance(s)).",
                to + 1,
                n
            );
            return;
        }
        if index == to {
            self.status = format!(
                "Sprite {} is already at position {}.",
                index + 1,
                to + 1
            );
            return;
        }
        self.snapshot_labeled("Move sprite");
        let inst = self.project.sprites.remove(index);
        self.project.sprites.insert(to, inst);
        self.status = format!(
            "Sprite moved from position {} to position {}.",
            index + 1,
            to + 1
        );
    }

    /// The sampled transform of sprite `index` at
    /// fractional frame `frame_f` — `InstanceKey::sample`
    /// over the instance's key list (interpolated between
    /// keys, clamped outside the keyed range). `None`
    /// when the sprite does not exist or has no keys.
    /// Read-only: never snapshots, never sets the status.
    pub fn sprite_sample(&self, index: usize, frame_f: f32) -> Option<InstanceKey> {
        let inst = self.project.sprites.get(index)?;
        InstanceKey::sample(&inst.keys, frame_f)
    }

    /// The STORED key of sprite `index` at exactly
    /// `frame`, if one was written there — as opposed to
    /// [`EditorCore::sprite_sample`], which interpolates.
    /// This is the read a panel's per-key steppers need:
    /// the values at the current frame are an authored
    /// key only when this returns `Some`. Read-only.
    pub fn sprite_key_at(&self, index: usize, frame: usize) -> Option<InstanceKey> {
        let inst = self.project.sprites.get(index)?;
        let frame_f = frame as f32;
        inst.keys.iter().find(|k| k.frame == frame_f).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ImageAsset;

    /// A core whose asset table holds `assets` dummy
    /// entries (names only — the ops never decode the
    /// bytes) and whose sprite table holds `sprites`.
    /// Everything else is the demo project: stage
    /// 960×720, five frames.
    fn core_with(assets: usize, sprites: Vec<SpriteInstance>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.assets = (0..assets)
            .map(|i| ImageAsset {
                name: format!("asset-{i}.png"),
                png: Vec::new(),
            })
            .collect();
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

    /// A key carrying the P-6 enrichment, for the write /
    /// preservation pins.
    fn enriched_key(frame: f32) -> InstanceKey {
        InstanceKey {
            opacity: 0.5,
            flip_x: true,
            flip_y: false,
            scale_y: 0.25,
            pivot_x: 0.25,
            pivot_y: 0.75,
            ..key(frame, 10.0, 20.0, 2.0, 30.0)
        }
    }

    fn sprite(asset: usize, keys: Vec<InstanceKey>) -> SpriteInstance {
        SpriteInstance { asset, keys }
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the `particle_ops` pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn add_sprite_lands_at_stage_centre_with_one_frame_zero_key() {
        let mut core = core_with(2, vec![]);
        let edits = core.edits_since_autosave();
        let index = core.add_sprite(1);
        assert_eq!(index, Some(0));
        assert_eq!(core.project.sprites.len(), 1);
        let inst = &core.project.sprites[0];
        assert_eq!(inst.asset, 1);
        assert_eq!(inst.keys.len(), 1, "one key at birth");
        let k = inst.keys[0];
        assert_eq!(k.frame, 0.0);
        assert_eq!((k.x, k.y), (480.0, 360.0), "the 960×720 stage centre");
        assert_eq!((k.scale, k.angle), (1.0, 0.0));
        assert_eq!(core.status, "Sprite 1 added (asset 2).");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn add_sprite_appends_and_returns_the_new_index() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 1.0, 2.0, 1.0, 0.0)])]);
        let index = core.add_sprite(0);
        assert_eq!(index, Some(1));
        assert_eq!(core.project.sprites.len(), 2);
        assert_eq!(core.status, "Sprite 2 added (asset 1).");
        // The pre-existing instance is untouched.
        assert_eq!(core.project.sprites[0].keys[0].x, 1.0);
    }

    #[test]
    fn add_sprite_refuses_a_missing_asset_without_snapshotting() {
        let mut core = core_with(1, vec![]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        assert_eq!(core.add_sprite(1), None);
        assert_eq!(core.status, "Sprite add failed: no asset 2.");
        assert!(core.project.sprites.is_empty());
        assert_eq!(project_json(&core), before, "a refusal changes nothing");
        assert_eq!(core.edits_since_autosave(), edits, "a refusal spends no snapshot");
    }

    #[test]
    fn add_sprite_is_one_undo_step() {
        let mut core = core_with(1, vec![]);
        core.add_sprite(0);
        assert_eq!(core.undo_depth(), 1);
        core.undo();
        assert!(core.project.sprites.is_empty(), "one undo removes the add whole");
    }

    #[test]
    fn remove_sprite_deletes_in_one_undo_step() {
        let mut core = core_with(
            1,
            vec![
                sprite(0, vec![key(0.0, 10.0, 10.0, 1.0, 0.0)]),
                sprite(0, vec![key(0.0, 20.0, 20.0, 1.0, 0.0)]),
            ],
        );
        let edits = core.edits_since_autosave();
        core.remove_sprite(0);
        assert_eq!(core.project.sprites.len(), 1);
        assert_eq!(
            core.project.sprites[0].keys[0].x, 20.0,
            "the survivor is the second instance"
        );
        assert_eq!(core.status, "Sprite 1 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn remove_sprite_refuses_a_missing_sprite_without_snapshotting() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.remove_sprite(3);
        assert_eq!(core.status, "Sprite delete failed: no sprite 4.");
        assert_eq!(project_json(&core), before);
        assert_eq!(core.edits_since_autosave(), edits);
    }

    #[test]
    fn set_sprite_key_inserts_in_frame_order() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 480.0, 360.0, 1.0, 0.0)])]);
        let edits = core.edits_since_autosave();
        core.set_sprite_key(0, 4, 600.0, 300.0, 2.0, 90.0);
        core.set_sprite_key(0, 2, 540.0, 330.0, 1.5, 45.0);
        let frames: Vec<f32> = core.project.sprites[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 2.0, 4.0], "keys stay sorted by frame");
        let mid = core.project.sprites[0].keys[1];
        assert_eq!((mid.x, mid.y, mid.scale, mid.angle), (540.0, 330.0, 1.5, 45.0));
        assert_eq!(core.status, "Sprite 1 key set at frame 3.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per call");
    }

    #[test]
    fn set_sprite_key_replaces_the_key_at_the_same_frame() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 480.0, 360.0, 1.0, 0.0)])]);
        core.set_sprite_key(0, 0, 100.0, 120.0, 0.5, -30.0);
        let keys = &core.project.sprites[0].keys;
        assert_eq!(keys.len(), 1, "a same-frame write replaces, never duplicates");
        assert_eq!(
            (keys[0].x, keys[0].y, keys[0].scale, keys[0].angle),
            (100.0, 120.0, 0.5, -30.0)
        );
    }

    #[test]
    fn set_sprite_key_clamps_the_frame_to_the_timeline() {
        let mut core = core_with(1, vec![sprite(0, vec![])]);
        let last = core.frame_count() - 1;
        core.set_sprite_key(0, 999, 10.0, 10.0, 1.0, 0.0);
        assert_eq!(core.project.sprites[0].keys[0].frame, last as f32);
        assert_eq!(
            core.status,
            format!("Sprite 1 key set at frame {}.", last + 1)
        );
    }

    #[test]
    fn set_sprite_key_refusals_spend_no_snapshot() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        // No such sprite.
        core.set_sprite_key(2, 0, 0.0, 0.0, 1.0, 0.0);
        assert_eq!(core.status, "Sprite key failed: no sprite 3.");
        // Non-finite values, each field in turn.
        for (x, y, scale, angle) in [
            (f32::NAN, 0.0, 1.0, 0.0),
            (0.0, f32::INFINITY, 1.0, 0.0),
            (0.0, 0.0, f32::NEG_INFINITY, 0.0),
            (0.0, 0.0, 1.0, f32::NAN),
        ] {
            core.set_sprite_key(0, 0, x, y, scale, angle);
            assert_eq!(core.status, "Sprite key failed: values must be finite.");
        }
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.edits_since_autosave(), edits, "refusals spend no snapshot");
    }

    #[test]
    fn delete_sprite_key_removes_only_that_key() {
        let mut core = core_with(
            1,
            vec![sprite(
                0,
                vec![
                    key(0.0, 0.0, 0.0, 1.0, 0.0),
                    key(2.0, 50.0, 50.0, 1.0, 0.0),
                    key(4.0, 90.0, 90.0, 1.0, 0.0),
                ],
            )],
        );
        let edits = core.edits_since_autosave();
        core.delete_sprite_key(0, 2);
        let frames: Vec<f32> = core.project.sprites[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 4.0]);
        assert_eq!(core.status, "Sprite 1 key at frame 3 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn delete_sprite_key_may_empty_the_key_list() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 5.0, 5.0, 1.0, 0.0)])]);
        core.delete_sprite_key(0, 0);
        assert_eq!(core.project.sprites.len(), 1, "the instance itself stays");
        assert!(core.project.sprites[0].keys.is_empty());
        assert!(
            core.sprite_sample(0, 0.0).is_none(),
            "an instance with no keys samples to nothing"
        );
    }

    #[test]
    fn delete_sprite_key_refusals_spend_no_snapshot() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.delete_sprite_key(1, 0);
        assert_eq!(core.status, "Sprite key delete failed: no sprite 2.");
        core.delete_sprite_key(0, 3);
        assert_eq!(
            core.status,
            "Sprite key delete failed: sprite 1 has no key at frame 4."
        );
        assert_eq!(project_json(&core), before);
        assert_eq!(core.edits_since_autosave(), edits);
    }

    #[test]
    fn delete_sprite_key_past_the_end_refuses_and_keeps_the_last_key() {
        // Wave P (P38): the clamped-delete hazard — a
        // delete naming a frame at/past the frame count
        // used to clamp onto the last frame and remove
        // ITS key. The op now refuses in the family
        // voice, spends nothing, and the last frame's
        // key survives.
        let mut core = core_with(
            1,
            vec![sprite(
                0,
                vec![
                    key(0.0, 0.0, 0.0, 1.0, 0.0),
                    key(4.0, 90.0, 90.0, 1.0, 0.0),
                ],
            )],
        );
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        // The first frame past the end (the demo project
        // has five frames), and one far past it.
        for frame in [5usize, 999] {
            core.delete_sprite_key(0, frame);
            assert_eq!(
                core.status,
                format!(
                    "Sprite key delete failed: frame {} is past the last frame (project has 5 frame(s)).",
                    frame + 1
                )
            );
        }
        assert_eq!(project_json(&core), before, "the last frame's key survives");
        assert_eq!(core.edits_since_autosave(), edits, "a refusal spends no snapshot");
        // The boundary itself still deletes: frame 4 IS
        // the last frame, not past it.
        core.delete_sprite_key(0, 4);
        assert_eq!(core.status, "Sprite 1 key at frame 5 deleted.");
        let frames: Vec<f32> = core.project.sprites[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0]);
    }

    #[test]
    fn sprite_sample_interpolates_and_clamps() {
        let core = core_with(
            1,
            vec![sprite(
                0,
                vec![
                    key(0.0, 100.0, 100.0, 1.0, 0.0),
                    key(4.0, 300.0, 200.0, 3.0, 90.0),
                ],
            )],
        );
        let mid = core.sprite_sample(0, 2.0).expect("a sample between keys");
        assert_eq!((mid.x, mid.y, mid.scale, mid.angle), (200.0, 150.0, 2.0, 45.0));
        let before = core.sprite_sample(0, -5.0).expect("clamped to the first key");
        assert_eq!((before.x, before.scale), (100.0, 1.0));
        let after = core.sprite_sample(0, 99.0).expect("clamped to the last key");
        assert_eq!((after.x, after.scale), (300.0, 3.0));
        assert!(core.sprite_sample(9, 0.0).is_none(), "no such sprite");
    }

    #[test]
    fn sprite_key_at_reads_stored_keys_only() {
        let core = core_with(
            1,
            vec![sprite(
                0,
                vec![
                    key(0.0, 100.0, 100.0, 1.0, 0.0),
                    key(4.0, 300.0, 200.0, 3.0, 90.0),
                ],
            )],
        );
        assert_eq!(core.sprite_key_at(0, 4).map(|k| k.x), Some(300.0));
        assert!(
            core.sprite_key_at(0, 2).is_none(),
            "frame 2 is interpolated by sampling but holds no stored key"
        );
        assert!(core.sprite_key_at(9, 0).is_none(), "no such sprite");
    }

    #[test]
    fn set_sprite_asset_repoints_and_keeps_every_key() {
        let mut core = core_with(
            2,
            vec![sprite(
                0,
                vec![
                    key(0.0, 100.0, 100.0, 1.0, 0.0),
                    key(4.0, 300.0, 200.0, 3.0, 90.0),
                ],
            )],
        );
        let edits = core.edits_since_autosave();
        core.set_sprite_asset(0, 1);
        let inst = &core.project.sprites[0];
        assert_eq!(inst.asset, 1);
        assert_eq!(inst.keys.len(), 2, "the keys are the point of the op");
        assert_eq!(
            (inst.keys[1].x, inst.keys[1].y, inst.keys[1].scale, inst.keys[1].angle),
            (300.0, 200.0, 3.0, 90.0),
            "the last key travels verbatim"
        );
        assert_eq!(core.status, "Sprite 1 now uses asset 2.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        core.undo();
        assert_eq!(core.project.sprites[0].asset, 0, "one undo restores the asset");
    }

    #[test]
    fn set_sprite_asset_refusals_and_no_op_spend_no_snapshot() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.set_sprite_asset(2, 0);
        assert_eq!(core.status, "Sprite asset failed: no sprite 3.");
        core.set_sprite_asset(0, 4);
        assert_eq!(core.status, "Sprite asset failed: no asset 5.");
        // Naming the asset already in use is a validated
        // no-op, not an undo step.
        core.set_sprite_asset(0, 0);
        assert_eq!(core.status, "Sprite 1 already uses asset 1.");
        assert_eq!(project_json(&core), before, "nothing was written");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot spent");
    }

    #[test]
    fn reorder_sprite_moves_the_instance_and_shifts_the_rest() {
        let mut core = core_with(
            1,
            vec![
                sprite(0, vec![key(0.0, 10.0, 0.0, 1.0, 0.0)]),
                sprite(0, vec![key(0.0, 20.0, 0.0, 1.0, 0.0)]),
                sprite(0, vec![key(0.0, 30.0, 0.0, 1.0, 0.0)]),
            ],
        );
        let edits = core.edits_since_autosave();
        // Bottom to top (position 0 -> 2): draw order is
        // vec order, so the moved instance now paints last.
        core.reorder_sprite(0, 2);
        let xs: Vec<f32> = core
            .project
            .sprites
            .iter()
            .map(|s| s.keys[0].x)
            .collect();
        assert_eq!(xs, vec![20.0, 30.0, 10.0], "the rest keep their relative order");
        assert_eq!(core.status, "Sprite moved from position 1 to position 3.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Top back to the middle.
        core.reorder_sprite(2, 1);
        let xs: Vec<f32> = core
            .project
            .sprites
            .iter()
            .map(|s| s.keys[0].x)
            .collect();
        assert_eq!(xs, vec![20.0, 10.0, 30.0]);
        core.undo();
        core.undo();
        let xs: Vec<f32> = core
            .project
            .sprites
            .iter()
            .map(|s| s.keys[0].x)
            .collect();
        assert_eq!(xs, vec![10.0, 20.0, 30.0], "two undos restore the order");
    }

    #[test]
    fn reorder_sprite_refusals_and_no_op_spend_no_snapshot() {
        let mut core = core_with(
            1,
            vec![
                sprite(0, vec![key(0.0, 10.0, 0.0, 1.0, 0.0)]),
                sprite(0, vec![key(0.0, 20.0, 0.0, 1.0, 0.0)]),
            ],
        );
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.reorder_sprite(5, 0);
        assert_eq!(core.status, "Sprite reorder failed: no sprite 6.");
        core.reorder_sprite(0, 2);
        assert_eq!(
            core.status,
            "Sprite reorder failed: no position 3 (project has 2 sprite instance(s))."
        );
        core.reorder_sprite(1, 1);
        assert_eq!(core.status, "Sprite 2 is already at position 2.");
        assert_eq!(project_json(&core), before, "nothing was written");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot spent");
    }

    #[test]
    fn sampling_helpers_are_read_only() {
        let core = core_with(1, vec![sprite(0, vec![key(0.0, 1.0, 1.0, 1.0, 0.0)])]);
        let status = core.status.clone();
        let _ = core.sprite_sample(0, 0.0);
        let _ = core.sprite_key_at(0, 0);
        assert_eq!(core.status, status, "reads never touch the status line");
        assert_eq!(core.undo_depth(), 0, "reads never snapshot");
    }

    #[test]
    fn set_sprite_key_preserves_the_replaced_keys_enrichment() {
        // Wave X (P-6): the transform write steps x/y/scale/
        // angle only — an imported sprite's flips, opacity,
        // scale_y and pivot survive a replace untouched.
        let mut core = core_with(1, vec![sprite(0, vec![enriched_key(0.0)])]);
        core.set_sprite_key(0, 0, 99.0, 88.0, 3.0, 45.0);
        let k = core.sprite_key_at(0, 0).expect("key still at frame 0");
        assert_eq!((k.x, k.y, k.scale, k.angle), (99.0, 88.0, 3.0, 45.0));
        assert_eq!(k.opacity, 0.5, "opacity preserved");
        assert!(k.flip_x && !k.flip_y, "flips preserved");
        assert_eq!(k.scale_y, 0.25, "scale_y preserved");
        assert_eq!((k.pivot_x, k.pivot_y), (0.25, 0.75), "pivot preserved");
        // A fresh insert (no key at the frame) authors the
        // defaults instead.
        core.set_sprite_key(0, 1, 1.0, 2.0, 1.0, 0.0);
        let fresh = core.sprite_key_at(0, 1).expect("inserted key");
        assert_eq!(fresh.opacity, 1.0);
        assert!(!fresh.flip_x && !fresh.flip_y);
        assert_eq!(fresh.scale_y, 0.0, "follow-scale default");
        assert_eq!((fresh.pivot_x, fresh.pivot_y), (0.5, 0.5));
    }

    #[test]
    fn set_sprite_key_full_writes_every_field_in_one_undo_step() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        core.set_sprite_key_full(0, 0, enriched_key(0.0));
        assert_eq!(core.undo_depth(), 1, "one snapshot for the whole key");
        let k = core.sprite_key_at(0, 0).expect("key at frame 0");
        assert_eq!((k.x, k.y, k.scale, k.angle), (10.0, 20.0, 2.0, 30.0));
        assert_eq!(k.opacity, 0.5);
        assert!(k.flip_x && !k.flip_y);
        assert_eq!(k.scale_y, 0.25);
        assert_eq!((k.pivot_x, k.pivot_y), (0.25, 0.75));
        assert_eq!(core.status, "Sprite 1 key set at frame 1.");
        core.undo();
        let back = core.sprite_key_at(0, 0).expect("original key restored");
        assert_eq!(back.opacity, 1.0, "one undo restores the plain key");
        assert_eq!(back.scale_y, 0.0);
    }

    #[test]
    fn set_sprite_key_full_refusals_spend_no_snapshot() {
        let mut core = core_with(1, vec![sprite(0, vec![key(0.0, 0.0, 0.0, 1.0, 0.0)])]);
        core.set_sprite_key_full(9, 0, enriched_key(0.0));
        assert_eq!(core.status, "Sprite key failed: no sprite 10.");
        let mut bad = enriched_key(0.0);
        bad.opacity = f32::NAN;
        core.set_sprite_key_full(0, 0, bad);
        assert_eq!(core.status, "Sprite key failed: values must be finite.");
        assert_eq!(core.undo_depth(), 0, "refusals never snapshot");
        let k = core.sprite_key_at(0, 0).expect("original key untouched");
        assert_eq!(k.opacity, 1.0);
    }
}
