//! Sprites panel (Wave 13, A3) — the GUI surface for
//! free-floating sprite instances (`Project.sprites`).
//!
//! Declared from `main.rs` only (bin-crate code, not part
//! of the engine library). Integration expectation:
//! `rune_sec/mod.rs` gains `pub mod sprites_panel;`,
//! `State` gains a `pub sprites_panel: SpritesPanelState`
//! field (initialised with `SpritesPanelState::new()`),
//! and the section builder is [`sprites_panel`].
//!
//! ## Entering truth (Wave 12, B5 — source-proven)
//!
//! Before this wave the project had NO sprite surface:
//! `SpriteInstance`s entered only via `.nodes` import,
//! composited into exports (`media::composite_media`)
//! and drew on the Rune stage as placeholder outlines
//! only; no shell control could select, move or delete
//! one (the Scene panel's placed-media note said exactly
//! that). The pattern mirrored here is the text tool's
//! free-standing-instance machinery (`text_tool.rs`):
//! a list of instances with select + delete, an
//! add path, and an edit area acting on the selected
//! instance at the current frame. Sprites differ from
//! texts in one way that shapes this panel: a sprite
//! has no content, size or colour fields of its own —
//! everything editable lives in its transform keys
//! (`InstanceKey { frame, x, y, scale, angle }`,
//! model.rs), so the edit area is four steppers
//! (x, y, rotation, scale) plus key add/delete.
//!
//! ## The A1 contract this panel codes against
//!
//! All project mutation goes through Wave 13 A1's new
//! engine ops (`src/sprite_ops.rs`, an `impl EditorCore`
//! block — the `figure_ops` / `audio_ops` idiom). That
//! file was NOT in the tree when this panel was written;
//! the panel calls exactly these four signatures, each
//! in exactly one wrapper below ([`add_from_asset`],
//! [`delete_sprite_row`], [`step_selected`] /
//! [`add_key_selected`], [`delete_key_selected`]), so
//! the signature drift (A1 landed scalar arguments where
//! this panel first assumed an `InstanceKey`) was
//! reconciled in those four places at integration:
//!
//! - `core.add_sprite(asset: usize)` — append an
//!   instance of `asset` at the stage centre with one
//!   key at frame 0. The panel detects success by the
//!   sprite vec growing (the house statement-call
//!   idiom: core ops set `core.status` and return
//!   nothing the panel needs), and selects the new
//!   instance when it does.
//! - `core.remove_sprite(index: usize)` — remove the
//!   instance at `index`.
//! - `core.set_sprite_key(index: usize, frame: usize,
//!   x: f32, y: f32, scale: f32, angle: f32)` — upsert a
//!   key at `frame` (replace an existing key at that
//!   frame, insert keeping ascending frame order
//!   otherwise; the frame clamps to the project's last
//!   frame). The wrappers below pass their built
//!   `InstanceKey`'s fields in this scalar shape —
//!   A1's landed signature, reconciled at integration.
//! - `core.delete_sprite_key(index: usize, frame:
//!   usize)` — remove the key at `frame`, if one exists.
//!
//! Wave X (X5, SpriteLoop GAP P-3) extends the contract
//! with the two instance-level ops in the same file,
//! each called from exactly one wrapper below
//! ([`set_asset_selected`], [`move_selected`]):
//!
//! - `core.set_sprite_asset(index: usize, asset:
//!   usize)` — re-point the instance at another asset,
//!   keys verbatim; naming its current asset is a
//!   validated no-op (no snapshot).
//! - `core.reorder_sprite(index: usize, to: usize)` —
//!   move the instance to a new table position (draw
//!   order is vec order, so this is the z-order);
//!   naming its current position is a validated no-op.
//!   The selection follows the moved instance via
//!   [`selected_after_reorder`].
//!
//! Per the roster, A1's ops take exactly one
//! `snapshot()` per mutating press and refuse
//! status-only (no snapshot) on stale indices and bad
//! assets, so every press here is at most one undo
//! step and a refusal changes nothing. Panel-side
//! pre-checks own the refusals the panel can see
//! coming (nothing selected, a key already at this
//! frame, no key at this frame to delete) with this
//! file's own status strings, pinned by its tests.
//!
//! ## Editing model
//!
//! - **Current frame** means the authored frame,
//!   `core.current` — the frame every other key editor
//!   in the shell writes at. (The stage *draws* the
//!   fractional `display_frame()` while playing with
//!   tween playback; authored keys are whole frames.)
//! - **Steppers upsert.** One press writes a key at
//!   the current frame holding the sampled transform
//!   with one field stepped — so stepping at a frame
//!   with no key creates the key there, carrying the
//!   other three fields at their sampled values (the
//!   track's shape elsewhere is untouched). The base
//!   is [`base_key`]: an exact key wins, else
//!   `InstanceKey::sample`, else — a keyless instance
//!   — the stage-centre default A1's add uses.
//! - **Rotation is degrees.** A sprite key's `angle`
//!   is stored in degrees (media.rs converts with
//!   `to_radians()` when compositing; `.nodes` import
//!   writes degrees). The step is 15°, labelled as
//!   such. (The text tool's rotate adds radians to
//!   *text* keys — a different field history; this
//!   panel follows the sprite data, not that code.)
//! - **Scale** steps 0.1, clamped to
//!   [`SCALE_MIN`]..=[`SCALE_MAX`]: the compositor
//!   skips scale ≤ 0 instances entirely, so the panel
//!   never writes a scale that would make the sprite
//!   vanish, and the cap keeps a held button from
//!   authoring an absurd quad. At a clamp the +/−
//!   button renders inert (the text tool's `step_btn`
//!   discipline), so the press cannot happen.
//! - **Selection is panel state.** Row clicks write
//!   `SpritesPanelState::selected` only — no snapshot,
//!   no status (the inspector's joint-navigation
//!   discipline). Deletion fixes the index up with
//!   [`selected_after_removal`] so it keeps naming
//!   the same instance.
//! - **The asset picker is uncapped.** The segment
//!   inspector caps its picker at 8 with a cycle
//!   button reaching the rest; a cycle makes sense
//!   for *replacing* one attachment, not for *adding*
//!   instances, so a cap here would strand assets
//!   past it with no way to place them. The picker
//!   wraps (`flex_wrap`, the fx_browser idiom) and
//!   shows every asset, its label truncated by
//!   [`sprite_label`].
//!
//! ## Design-kit styling (Phase N)
//!
//! The root is [`design::panel_col`] with a
//! [`design::panel_title`] header (the `image` glyph)
//! over a [`design::divider`]; the three areas are
//! [`design::section_label`]s; instance rows sit on
//! [`design::list_row`] shells (the selected sprite's
//! row carries the selected fill); stepper readouts
//! render through [`design::stepper_value`]; the two
//! empty lists render through [`design::empty_state`].
//! Edit-area buttons gate like the text tool's: live
//! [`btn`] while a sprite is selected, the muted-live
//! form with the handler kept (a press earns the
//! "Select a sprite first." refusal) while none is.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{InstanceKey, SpriteInstance};

use crate::chrome_view::{btn, btn_inert, btn_muted_live};
use crate::rune_sec::design;
use crate::State;

/// X/Y stepper step, in stage units.
pub const POS_STEP: f32 = 10.0;
/// Rotation stepper step, in degrees (sprite keys
/// store `angle` in degrees — see the module docs).
pub const ROT_STEP_DEG: f32 = 15.0;
/// Scale stepper step (a multiplier on the image's
/// pixel size, as `media_items` applies it).
pub const SCALE_STEP: f32 = 0.1;
/// Smallest scale a stepper press can write: the
/// compositor skips scale ≤ 0, so the panel floor
/// keeps every written key visible.
pub const SCALE_MIN: f32 = 0.1;
/// Largest scale a stepper press can write.
pub const SCALE_MAX: f32 = 8.0;

/// How many characters of an asset name a row or
/// picker label shows before truncating.
pub const LABEL_CHARS: usize = 18;

/// View state for the sprites panel: which instance
/// the edit area acts on. All engine state stays in
/// `core.project.sprites`; this is selection only.
pub struct SpritesPanelState {
    /// Index into `Project.sprites` of the instance
    /// the last row click selected, if any. Row
    /// deletions adjust it so it keeps naming the
    /// same instance ([`selected_after_removal`]).
    pub selected: Option<usize>,
}

impl Default for SpritesPanelState {
    fn default() -> Self {
        SpritesPanelState { selected: None }
    }
}

impl SpritesPanelState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// One placed sprite instance, for the list: its real
/// index in the vec (the row's buttons act on the
/// instance it names, even as deletions shift the
/// indices of the rest), the asset it draws, that
/// asset's name (or the missing-asset fallback), and
/// how many transform keys it carries.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteRow {
    pub index: usize,
    pub asset: usize,
    pub asset_name: String,
    pub missing: bool,
    pub keys: usize,
}

/// One project asset the Add picker can place: its
/// index in `Project.assets` and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetOption {
    pub index: usize,
    pub name: String,
}

/// The display name of asset `asset`: its stored
/// name, or — for a sprite whose asset was removed
/// from under it (the table can shrink via MCP
/// remapping paths the panel does not control) — the
/// fallback the segment inspector's media readout
/// uses, so a broken reference reads as broken
/// instead of panicking or borrowing another name.
pub fn asset_name(core: &EditorCore, asset: usize) -> String {
    match core.project.assets.get(asset) {
        Some(a) => a.name.clone(),
        None => format!("asset {} (missing)", asset + 1),
    }
}

/// A row/picker label for `name`: the name itself
/// when it fits in [`LABEL_CHARS`] characters,
/// otherwise its first `LABEL_CHARS` characters plus
/// an ellipsis. Counts characters, never bytes, so
/// multi-byte names truncate cleanly (the text tool's
/// preview discipline). Pure.
pub fn sprite_label(name: &str) -> String {
    if name.chars().count() <= LABEL_CHARS {
        return name.to_string();
    }
    let head: String = name.chars().take(LABEL_CHARS).collect();
    format!("{head}…")
}

/// All placed sprite instances in the project, in
/// vec order (which is also draw order: later rows
/// paint over earlier ones).
pub fn sprite_rows(core: &EditorCore) -> Vec<SpriteRow> {
    core.project
        .sprites
        .iter()
        .enumerate()
        .map(|(index, inst)| SpriteRow {
            index,
            asset: inst.asset,
            missing: core.project.assets.get(inst.asset).is_none(),
            asset_name: asset_name(core, inst.asset),
            keys: inst.keys.len(),
        })
        .collect()
}

/// All assets the Add picker offers, in table order.
pub fn asset_options(core: &EditorCore) -> Vec<AssetOption> {
    core.project
        .assets
        .iter()
        .enumerate()
        .map(|(index, a)| AssetOption {
            index,
            name: a.name.clone(),
        })
        .collect()
}

/// The frame the edit area writes keys at: the
/// authored current frame as the model's fractional
/// frame type. Pure read of `core.current`.
pub fn current_frame(core: &EditorCore) -> f32 {
    core.current as f32
}

/// Frame equality for key lookup: keys are authored
/// at whole frames and stored as `f32`, so a small
/// epsilon absorbs import round-trips (`.nodes`
/// writes fractional frames as text) without ever
/// matching a neighbouring frame. Pure.
fn frame_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

/// The instance's key exactly at `frame`, if it has
/// one. Pure.
pub fn exact_key(inst: &SpriteInstance, frame: f32) -> Option<InstanceKey> {
    inst.keys.iter().find(|k| frame_eq(k.frame, frame)).copied()
}

/// The transform the edit area starts from at
/// `frame`: an exact key at the frame wins; else the
/// sampled (interpolated) transform; else — a
/// keyless instance — the stage-centre default with
/// scale 1 and no rotation, the placement A1's
/// `add_sprite` authors. The returned key always
/// carries `frame`: it is the value a write at this
/// frame would store, before any stepping. Pure.
pub fn base_key(core: &EditorCore, inst: &SpriteInstance, frame: f32) -> InstanceKey {
    if let Some(k) = exact_key(inst, frame) {
        return k;
    }
    if let Some(k) = InstanceKey::sample(&inst.keys, frame) {
        return InstanceKey { frame, ..k };
    }
    InstanceKey {
        frame,
        x: core.project.stage[0] / 2.0,
        y: core.project.stage[1] / 2.0,
        scale: 1.0,
        angle: 0.0,
        opacity: 1.0,
        flip_x: false,
        flip_y: false,
        scale_y: 0.0,
        pivot_x: 0.5,
        pivot_y: 0.5,
    }
}

/// Which transform field a stepper press steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteField {
    X,
    Y,
    Rotation,
    Scale,
}

/// `base` with one field stepped by `dir` (−1/+1)
/// and its frame set to `frame`: x/y move by
/// [`POS_STEP`] stage units, rotation turns by
/// [`ROT_STEP_DEG`] degrees, scale multiplies its
/// step in [`SCALE_STEP`] increments clamped to
/// [`SCALE_MIN`]..=[`SCALE_MAX`]. The other three
/// fields carry over untouched, so an upsert at a
/// fresh frame preserves the sampled transform
/// except for the stepped field. Pure.
pub fn stepped_key(base: InstanceKey, frame: f32, field: SpriteField, dir: i32) -> InstanceKey {
    let mut key = InstanceKey { frame, ..base };
    match field {
        SpriteField::X => key.x += dir as f32 * POS_STEP,
        SpriteField::Y => key.y += dir as f32 * POS_STEP,
        SpriteField::Rotation => key.angle += dir as f32 * ROT_STEP_DEG,
        SpriteField::Scale => {
            key.scale = (key.scale + dir as f32 * SCALE_STEP).clamp(SCALE_MIN, SCALE_MAX)
        }
    }
    key
}

/// Whether one scale step from `scale` in `dir`
/// cannot change the scale (it sits at the clamp the
/// step pushes against), so the button renders
/// inert. An out-of-range stored scale is NOT a
/// no-op in either direction — each step clamps it
/// back inside the range, a real change (the text
/// tool's `step_is_noop` edge). Pure.
pub fn scale_step_is_noop(scale: f32, dir: i32) -> bool {
    let stepped = (scale + dir as f32 * SCALE_STEP).clamp(SCALE_MIN, SCALE_MAX);
    stepped == scale
}

/// Where a remembered `selected` index points after
/// the instance at `removed` is deleted: the
/// selected instance itself is gone (`None`); an
/// instance after the gap shifts down one; one
/// before it is unmoved. Pure. (The text tool's
/// `loaded_after_removal` math, kept locally — each
/// panel owns its selection fix-up.)
pub fn selected_after_removal(selected: Option<usize>, removed: usize) -> Option<usize> {
    match selected {
        Some(s) if s == removed => None,
        Some(s) if s > removed => Some(s - 1),
        other => other,
    }
}

/// Where a remembered `selected` index points after
/// the instance at `from` is moved to position `to`
/// (Wave X, X5 — the z-reorder's fix-up, the
/// [`selected_after_removal`] discipline for a move
/// instead of a removal): the moved instance's
/// selection follows it; an instance between the two
/// positions shifts by one toward `from`; anything
/// else is unmoved. Pure.
pub fn selected_after_reorder(
    selected: Option<usize>,
    from: usize,
    to: usize,
) -> Option<usize> {
    match selected {
        Some(s) if s == from => Some(to),
        Some(s) if from < s && s <= to => Some(s - 1),
        Some(s) if to <= s && s < from => Some(s + 1),
        other => other,
    }
}

/// The edit area's readout line for the transform
/// `key` at `frame`: whether it is an authored key
/// or the sampled value, then the four fields in
/// stepper order. Pure — the builder renders this
/// string verbatim, so the wording is pinned here.
pub fn transform_readout(key: &InstanceKey, frame: f32, exact: bool) -> String {
    let kind = if exact {
        format!("Key at frame {}", frame as usize + 1)
    } else {
        format!("Sampled at frame {} — no key here", frame as usize + 1)
    };
    format!(
        "{kind} · x {:.0} · y {:.0} · {:.0}° · ×{:.2}",
        key.x, key.y, key.angle, key.scale
    )
}

// ---------------------------------------------------------------------
// Handlers — the only callers of A1's ops (see the
// module docs for the assumed signatures).
// ---------------------------------------------------------------------

/// Row-click handler: select the instance at
/// `index`. A pure `SpritesPanelState` write — the
/// project is untouched, nothing is snapshotted, no
/// status is set. A stale index clears nothing and
/// selects nothing.
fn select_sprite(s: &mut State, index: usize) {
    if index < s.core.project.sprites.len() {
        s.sprites_panel.selected = Some(index);
    }
}

/// Add-picker handler: place a new instance of
/// `asset` via A1's `add_sprite`, then select the
/// instance it appended. Success is read off the vec
/// growing; when the op refuses (a stale asset) the
/// selection stays where it was and the op's status
/// stands.
fn add_from_asset(s: &mut State, asset: usize) {
    let before = s.core.project.sprites.len();
    s.core.add_sprite(asset);
    if s.core.project.sprites.len() > before {
        s.sprites_panel.selected = Some(before);
    }
}

/// Delete-button handler: keep `selected` naming
/// the same instance across A1's `remove_sprite`,
/// via [`selected_after_removal`].
fn delete_sprite_row(s: &mut State, index: usize) {
    if index < s.core.project.sprites.len() {
        s.sprites_panel.selected =
            selected_after_removal(s.sprites_panel.selected, index);
    }
    s.core.remove_sprite(index);
}

/// Asset-picker handler (edit area): re-point the
/// selected instance at `asset` via the
/// `set_sprite_asset` op (Wave X, X5 — SpriteLoop
/// GAP P-3). The op owns the refusals (a stale asset
/// cannot be picked — the picker is built from the
/// live table — but a stale selection refuses
/// there); the instance's keys are untouched by
/// construction.
fn set_asset_selected(s: &mut State, asset: usize) {
    let Some(i) = selected_index(s) else {
        return;
    };
    s.core.set_sprite_asset(i, asset);
}

/// Draw-order handler (edit area): move the selected
/// instance one step toward the front (`dir` +1 —
/// later in the table paints later, i.e. on top) or
/// the back (`dir` −1), via the `reorder_sprite` op
/// (Wave X, X5). The selection follows the instance
/// via [`selected_after_reorder`], applied only when
/// the op actually moved it (an in-range index and a
/// different target — the op's own success condition;
/// its refusal or no-op leaves the selection alone).
/// A boundary press cannot happen (the button
/// renders inert there); a direct call clamps its
/// target onto the boundary and earns the op's
/// validated no-op sentence instead of moving
/// anything.
fn move_selected(s: &mut State, dir: i32) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let n = s.core.project.sprites.len();
    if n == 0 {
        return;
    }
    let target = (i as i32 + dir).clamp(0, (n - 1) as i32) as usize;
    s.core.reorder_sprite(i, target);
    if i < n && target != i {
        s.sprites_panel.selected =
            selected_after_reorder(s.sprites_panel.selected, i, target);
    }
}

/// The selected instance's index, or a status-only
/// refusal when no row click has selected one. The
/// core ops bounds-check the index itself, so a
/// stale selection refuses there instead.
fn selected_index(s: &mut State) -> Option<usize> {
    match s.sprites_panel.selected {
        Some(i) => Some(i),
        None => {
            s.core.status = "Select a sprite first.".into();
            None
        }
    }
}

/// Stepper handler: step one field of the selected
/// instance's transform at the current frame, via
/// A1's `set_sprite_key` upsert. The value written
/// is [`stepped_key`] over [`base_key`], so a press
/// at a keyless frame authors the key there. A stale
/// selection refuses here ("No such sprite.") before
/// any op is called.
fn step_selected(s: &mut State, field: SpriteField, dir: i32) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i).cloned() else {
        s.core.status = "No such sprite.".into();
        return;
    };
    let key = stepped_key(base_key(&s.core, &inst, frame), frame, field, dir);
    s.core.set_sprite_key(i, key.frame as usize, key.x, key.y, key.scale, key.angle);
}

/// "Add key" handler: author a key at the current
/// frame holding the sampled transform unchanged,
/// via A1's `set_sprite_key`. When a key already
/// exists at this frame the press refuses here —
/// the sampled value IS that key, so the write
/// would only mint a phantom undo step.
fn add_key_selected(s: &mut State) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i).cloned() else {
        s.core.status = "No such sprite.".into();
        return;
    };
    if exact_key(&inst, frame).is_some() {
        s.core.status = format!("There is already a key at frame {}.", frame as usize + 1);
        return;
    }
    let key = base_key(&s.core, &inst, frame);
    s.core.set_sprite_key(i, key.frame as usize, key.x, key.y, key.scale, key.angle);
}

/// "Delete key" handler: remove the selected
/// instance's key at the current frame, via A1's
/// `delete_sprite_key`. With no key at this frame
/// the press refuses here (the button also renders
/// inert in that state, so this is the armour for
/// direct callers, not the expected path).
fn delete_key_selected(s: &mut State) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i) else {
        s.core.status = "No such sprite.".into();
        return;
    };
    if exact_key(inst, frame).is_none() {
        s.core.status = format!("No key at frame {} to delete.", frame as usize + 1);
        return;
    }
    s.core.delete_sprite_key(i, frame as usize);
}

/// One edit-area button, gated on a sprite actually
/// being selected (the text tool's `edit_btn`
/// discipline): fully live [`btn`] while one is;
/// with nothing selected, the chrome family's
/// muted-live member — the inert visual with the
/// handler KEPT, so a press still earns the pinned
/// "Select a sprite first." refusal from
/// [`selected_index`] instead of flashing like a
/// live control that then does nothing.
fn edit_btn(
    key: &str,
    label: &str,
    live: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    if live {
        btn(key, label, false, t, f)
    } else {
        btn_muted_live(key, label, false, t, f)
    }
}

/// One stepper button, three forms over one key and
/// box (the text tool's `step_btn` shape): not
/// `live` — the muted-live form via [`edit_btn`];
/// `live` but the step cannot change the value
/// (`noop` — only scale can, at its clamps) — the
/// handler-free [`btn_inert`]; otherwise [`btn`].
fn step_btn(
    key: &str,
    label: &str,
    live: bool,
    noop: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    // Wave P: delegates to the kit's `design::step_btn`
    // — the three forms above are its StepForm split
    // (not-live is MutedLive: `edit_btn` IS the
    // muted-live wrapper, handler kept for the refusal).
    let form = if !live {
        design::StepForm::MutedLive
    } else if noop {
        design::StepForm::Noop
    } else {
        design::StepForm::Live
    };
    design::step_btn(key, label, form, t, f)
}

/// One stepper row: the field's label, its − button,
/// the value readout ([`design::stepper_value`]),
/// its + button (the kit's F-STEP order).
fn stepper_row(
    row_key: &str,
    label: &str,
    dn_key: &str,
    dn_label: &str,
    up_key: &str,
    up_label: &str,
    value: &str,
    field: SpriteField,
    live: bool,
    noop_dn: bool,
    noop_up: bool,
    t: &Theme,
) -> El {
    row()
        .key(row_key)
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(step_btn(dn_key, dn_label, live, noop_dn, t, move |s: &mut State| {
            step_selected(s, field, -1)
        }))
        .child(design::stepper_value(value, t))
        .child(step_btn(up_key, up_label, live, noop_up, t, move |s: &mut State| {
            step_selected(s, field, 1)
        }))
}

/// The Sprites section: the placed instances (select
/// + delete), the add-from-assets picker, and the
/// selected instance's transform steppers + key
/// add/delete at the current frame.
pub fn sprites_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let rows = sprite_rows(core);
    let options = asset_options(core);
    let selected = s.sprites_panel.selected;
    let frame = current_frame(core);

    // The selected instance's edit-area data, resolved
    // against the live vec (a stale selection simply
    // renders the unselected form — the builder never
    // trusts the remembered index).
    let selected_row = rows.iter().find(|r| Some(r.index) == selected);
    let selected_inst = selected.and_then(|i| core.project.sprites.get(i));
    let base = selected_inst.map(|inst| base_key(core, inst, frame));
    let has_exact = selected_inst.map_or(false, |inst| exact_key(inst, frame).is_some());
    let edit_live = selected_row.is_some();

    // --- Instances list --------------------------------
    let mut list = col().key("sprite-list").gap(2.0);
    list = list.child(design::section_label("Placed sprites", t));
    if rows.is_empty() {
        list = list.child(design::empty_state(
            "image",
            "No sprites placed.",
            t,
        ));
    } else {
        for r in &rows {
            let i = r.index;
            list = list.child(
                design::list_row(&format!("sprite-row-{i}"), selected == Some(i), t)
                    .child(btn(
                        &format!("sprite-select-{i}"),
                        &sprite_label(&r.asset_name),
                        false,
                        t,
                        move |s: &mut State| select_sprite(s, i),
                    ))
                    .child(
                        text(format!(
                            "{} key{}",
                            r.keys,
                            if r.keys == 1 { "" } else { "s" }
                        ))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .truncate(),
                    )
                    .child(btn(
                        &format!("sprite-del-{i}"),
                        "Delete",
                        false,
                        t,
                        move |s: &mut State| delete_sprite_row(s, i),
                    )),
            );
        }
    }

    // --- Add picker ------------------------------------
    let mut add = col().key("sprite-add").gap(2.0);
    add = add.child(design::section_label("Add sprite", t));
    if options.is_empty() {
        add = add.child(
            design::empty_state("image", "No image assets in this project yet.", t)
                .key("sprite-add-empty"),
        );
    } else {
        let mut picker = row().gap(6.0).w_full().flex_wrap();
        for opt in &options {
            let a = opt.index;
            picker = picker.child(btn(
                &format!("sprite-add-{a}"),
                &sprite_label(&opt.name),
                false,
                t,
                move |s: &mut State| add_from_asset(s, a),
            ));
        }
        add = add.child(picker);
    }

    // --- Selected instance edit area -------------------
    let edit_label = match selected_row {
        Some(r) => format!("Selected: {}", sprite_label(&r.asset_name)),
        None => "No sprite selected — click a sprite above to select it.".to_string(),
    };
    let mut edit = col().key("sprite-edit").w_full().gap(2.0);
    edit = edit.child(design::section_label(&edit_label, t));
    if let (Some(_row), Some(key)) = (selected_row, base) {
        edit = edit.child(
            text(transform_readout(&key, frame, has_exact))
                .key("sprite-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    let (x_val, y_val, rot_val, scale_val) = match base {
        Some(k) => (
            format!("{:.0}", k.x),
            format!("{:.0}", k.y),
            format!("{:.0}°", k.angle),
            format!("{:.2}", k.scale),
        ),
        None => (
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
        ),
    };
    let scale_noop_dn = base.map_or(false, |k| scale_step_is_noop(k.scale, -1));
    let scale_noop_up = base.map_or(false, |k| scale_step_is_noop(k.scale, 1));

    // The selected instance's asset picker (Wave X,
    // X5): one button per asset — the Add picker's
    // shape, gated like the edit area (muted-live with
    // nothing selected) and with the instance's
    // current asset marked, since this picker REPLACES
    // rather than adds. Its keys keep their keys: the
    // op writes the asset field only.
    let current_asset = selected_inst.map(|inst| inst.asset);
    let mut asset_picker = col().key("sprite-asset-row").w_full().gap(2.0);
    asset_picker = asset_picker.child(design::section_label("Asset", t));
    {
        let mut picker = row().gap(6.0).w_full().flex_wrap();
        for opt in &options {
            let a = opt.index;
            picker = picker.child(if edit_live {
                btn(
                    &format!("sprite-asset-{a}"),
                    &sprite_label(&opt.name),
                    current_asset == Some(a),
                    t,
                    move |s: &mut State| set_asset_selected(s, a),
                )
            } else {
                btn_muted_live(
                    &format!("sprite-asset-{a}"),
                    &sprite_label(&opt.name),
                    false,
                    t,
                    move |s: &mut State| set_asset_selected(s, a),
                )
            });
        }
        asset_picker = asset_picker.child(picker);
    }

    // The draw-order row (Wave X, X5): the selected
    // instance's 1-based position of the table count,
    // one step each way. A step at the table's edge
    // renders inert (the step_btn discipline — the
    // press cannot happen); with nothing selected the
    // pair is muted-live and earns the refusal.
    let z_readout = match selected_row {
        Some(r) => format!("{} of {}", r.index + 1, rows.len()),
        None => "—".to_string(),
    };
    let at_back = matches!(selected_row, Some(r) if r.index == 0);
    let at_front = matches!(selected_row, Some(r) if r.index + 1 == rows.len());
    let z_row = row()
        .key("sprite-z-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("Draw order")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(step_btn(
            "sprite-z-back",
            "Send backward",
            edit_live,
            at_back,
            t,
            |s: &mut State| move_selected(s, -1),
        ))
        .child(design::stepper_value(&z_readout, t))
        .child(step_btn(
            "sprite-z-front",
            "Bring forward",
            edit_live,
            at_front,
            t,
            |s: &mut State| move_selected(s, 1),
        ));

    edit = edit
        .child(stepper_row(
            "sprite-x-row", "X",
            "sprite-x-dn", "X −", "sprite-x-up", "X +",
            &x_val, SpriteField::X, edit_live, false, false, t,
        ))
        .child(stepper_row(
            "sprite-y-row", "Y",
            "sprite-y-dn", "Y −", "sprite-y-up", "Y +",
            &y_val, SpriteField::Y, edit_live, false, false, t,
        ))
        .child(stepper_row(
            "sprite-rot-row", "Rotation",
            "sprite-rot-dn", "Rotate −15°", "sprite-rot-up", "Rotate +15°",
            &rot_val, SpriteField::Rotation, edit_live, false, false, t,
        ))
        .child(stepper_row(
            "sprite-scale-row", "Scale",
            "sprite-scale-dn", "Scale −", "sprite-scale-up", "Scale +",
            &scale_val, SpriteField::Scale, edit_live, scale_noop_dn, scale_noop_up, t,
        ))
        .child(
            row()
                .key("sprite-key-row")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(edit_btn(
                    "sprite-key-add",
                    "Add key",
                    edit_live,
                    t,
                    |s| add_key_selected(s),
                ))
                .child(if !edit_live {
                    edit_btn(
                        "sprite-key-del",
                        "Delete key",
                        false,
                        t,
                        |s| delete_key_selected(s),
                    )
                } else if has_exact {
                    btn("sprite-key-del", "Delete key", false, t, |s| {
                        delete_key_selected(s)
                    })
                } else {
                    btn_inert("sprite-key-del", "Delete key", t)
                }),
        );
    // The asset picker renders only when there is an
    // asset to pick — with an empty table it could
    // offer nothing, and the Add section above already
    // states that fact in its own empty state.
    if !options.is_empty() {
        edit = edit.child(asset_picker);
    }
    edit = edit.child(z_row);

    design::panel_col("sprite-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("image"),
                    "Sprites",
                    Some(&format!(
                        "{} placed",
                        rows.len()
                    )),
                    t,
                )),
        )
        .child(design::divider(t))
        .child(list)
        .child(add)
        .child(edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::ImageAsset;

    /// A core carrying `assets` named assets and
    /// `sprites` instances with the given key counts —
    /// the fixture every row/readout test reads from.
    fn core_with(assets: &[&str], sprites: &[(usize, usize)]) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.assets = assets
            .iter()
            .map(|name| ImageAsset {
                name: name.to_string(),
                png: Vec::new(),
            })
            .collect();
        core.project.sprites = sprites
            .iter()
            .map(|(asset, keys)| SpriteInstance {
                asset: *asset,
                keys: (0..*keys)
                    .map(|k| InstanceKey {
                        frame: k as f32 * 2.0,
                        x: 100.0 + k as f32 * 10.0,
                        y: 200.0,
                        scale: 1.0,
                        angle: 0.0,
                        opacity: 1.0,
                        flip_x: false,
                        flip_y: false,
                        scale_y: 0.0,
                        pivot_x: 0.5,
                        pivot_y: 0.5,
                    })
                    .collect(),
            })
            .collect();
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

    /// Field-by-field key equality: `InstanceKey`
    /// derives no `PartialEq` (model.rs), so tests
    /// compare the five fields, not the struct.
    fn assert_key(actual: InstanceKey, expected: InstanceKey) {
        assert_eq!(actual.frame, expected.frame, "frame");
        assert_eq!(actual.x, expected.x, "x");
        assert_eq!(actual.y, expected.y, "y");
        assert_eq!(actual.scale, expected.scale, "scale");
        assert_eq!(actual.angle, expected.angle, "angle");
    }

    fn assert_key_opt(actual: Option<InstanceKey>, expected: InstanceKey) {
        let actual = actual.expect("expected a key");
        assert_key(actual, expected);
    }

    #[test]
    fn sprite_rows_carry_index_asset_name_and_key_count() {
        let core = core_with(&["hero.png", "tree.png"], &[(1, 3), (0, 1)]);
        let rows = sprite_rows(&core);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[0].asset, 1);
        assert_eq!(rows[0].asset_name, "tree.png");
        assert!(!rows[0].missing);
        assert_eq!(rows[0].keys, 3);
        assert_eq!(rows[1].asset_name, "hero.png");
        assert_eq!(rows[1].keys, 1);
    }

    #[test]
    fn sprite_rows_name_a_missing_asset_as_missing() {
        let core = core_with(&["hero.png"], &[(7, 2)]);
        let rows = sprite_rows(&core);
        assert_eq!(rows[0].asset_name, "asset 8 (missing)");
        assert!(rows[0].missing, "the row flags the broken reference");
    }

    #[test]
    fn asset_options_follow_table_order() {
        let core = core_with(&["b.png", "a.png"], &[]);
        let options = asset_options(&core);
        assert_eq!(
            options,
            vec![
                AssetOption { index: 0, name: "b.png".into() },
                AssetOption { index: 1, name: "a.png".into() },
            ],
            "table order, not name order — the picker adds the asset it names"
        );
    }

    #[test]
    fn sprite_label_keeps_short_names_and_truncates_long_ones() {
        assert_eq!(sprite_label("dot.png"), "dot.png");
        let exact = "a".repeat(LABEL_CHARS);
        assert_eq!(sprite_label(&exact), exact, "exactly at the cap is whole");
        let long = "a".repeat(LABEL_CHARS + 4);
        assert_eq!(
            sprite_label(&long),
            format!("{}…", "a".repeat(LABEL_CHARS))
        );
        // Multi-byte names count characters, not bytes.
        let accents = "é".repeat(LABEL_CHARS + 2);
        assert_eq!(
            sprite_label(&accents),
            format!("{}…", "é".repeat(LABEL_CHARS))
        );
    }

    #[test]
    fn exact_key_matches_only_the_authored_frame() {
        let inst = SpriteInstance {
            asset: 0,
            keys: vec![key(0.0, 10.0, 20.0, 1.0, 0.0), key(4.0, 50.0, 60.0, 2.0, 30.0)],
        };
        assert_key_opt(exact_key(&inst, 4.0), key(4.0, 50.0, 60.0, 2.0, 30.0));
        assert!(exact_key(&inst, 2.0).is_none(), "between keys is sampled, not exact");
        assert_key_opt(exact_key(&inst, 0.0), key(0.0, 10.0, 20.0, 1.0, 0.0));
    }

    #[test]
    fn base_key_prefers_exact_then_sampled_then_centre_default() {
        let mut core = core_with(&["a.png"], &[(0, 2)]);
        // Fixture keys: frame 0 at x=100, frame 2 at x=110.
        let inst = core.project.sprites[0].clone();
        assert_eq!(base_key(&core, &inst, 2.0).x, 110.0, "the exact key wins");
        assert_key(
            base_key(&core, &inst, 1.0),
            key(1.0, 105.0, 200.0, 1.0, 0.0),
        );
        // (between keys the sample is re-framed to the
        // current frame — the frame field above is 1.0,
        // not either authored frame)
        let keyless = SpriteInstance { asset: 0, keys: Vec::new() };
        core.project.stage = [960.0, 720.0];
        assert_key(
            base_key(&core, &keyless, 3.0),
            key(3.0, 480.0, 360.0, 1.0, 0.0),
        );
        // (a keyless instance starts from the stage
        // centre — the assert above)
    }

    #[test]
    fn stepped_key_steps_one_field_and_reframes() {
        let base = key(0.0, 100.0, 200.0, 1.0, 30.0);
        assert_key(
            stepped_key(base, 5.0, SpriteField::X, 1),
            key(5.0, 100.0 + POS_STEP, 200.0, 1.0, 30.0),
        );
        assert_key(
            stepped_key(base, 5.0, SpriteField::Y, -1),
            key(5.0, 100.0, 200.0 - POS_STEP, 1.0, 30.0),
        );
        assert_key(
            stepped_key(base, 5.0, SpriteField::Rotation, 1),
            key(5.0, 100.0, 200.0, 1.0, 30.0 + ROT_STEP_DEG),
        );
        // (rotation steps in degrees — the sprite key's
        // storage unit; see the assert above)
        assert_key(
            stepped_key(base, 5.0, SpriteField::Scale, -1),
            key(5.0, 100.0, 200.0, 1.0 - SCALE_STEP, 30.0),
        );
    }

    #[test]
    fn stepped_key_clamps_scale_to_the_panel_range() {
        let big = key(0.0, 0.0, 0.0, SCALE_MAX, 0.0);
        assert_eq!(
            stepped_key(big, 0.0, SpriteField::Scale, 1).scale,
            SCALE_MAX,
            "at the cap, + stays at the cap"
        );
        let small = key(0.0, 0.0, 0.0, SCALE_MIN, 0.0);
        assert_eq!(
            stepped_key(small, 0.0, SpriteField::Scale, -1).scale,
            SCALE_MIN,
            "at the floor, − stays at the floor — never 0, which the compositor skips"
        );
    }

    #[test]
    fn scale_step_is_noop_only_at_the_clamp_it_pushes_against() {
        assert!(scale_step_is_noop(SCALE_MAX, 1));
        assert!(scale_step_is_noop(SCALE_MIN, -1));
        assert!(!scale_step_is_noop(SCALE_MAX, -1));
        assert!(!scale_step_is_noop(SCALE_MIN, 1));
        assert!(!scale_step_is_noop(1.0, 1));
        assert!(!scale_step_is_noop(100.0, 1), "an oversize stored scale clamps in — a real change");
        assert!(!scale_step_is_noop(0.0, -1), "an undersize stored scale clamps in — a real change");
    }

    #[test]
    fn selected_after_removal_follows_the_text_tool_math() {
        assert_eq!(selected_after_removal(None, 0), None);
        assert_eq!(selected_after_removal(Some(2), 2), None, "the selected sprite itself was deleted");
        assert_eq!(selected_after_removal(Some(3), 1), Some(2), "after the gap shifts down");
        assert_eq!(selected_after_removal(Some(0), 1), Some(0), "before the gap is unmoved");
    }

    #[test]
    fn transform_readout_names_key_vs_sampled_and_all_fields() {
        let k = key(2.0, 480.0, 360.0, 1.5, 45.0);
        assert_eq!(
            transform_readout(&k, 2.0, true),
            "Key at frame 3 · x 480 · y 360 · 45° · ×1.50"
        );
        assert_eq!(
            transform_readout(&k, 2.0, false),
            "Sampled at frame 3 — no key here · x 480 · y 360 · 45° · ×1.50"
        );
    }

    #[test]
    fn current_frame_is_the_authored_frame() {
        let mut core = EditorCore::new();
        core.current = 7;
        assert_eq!(current_frame(&core), 7.0);
    }

    // -- Handler tests: these pin the A1 contract the
    // module docs spell out. They compile and pass
    // once A1's sprite_ops land with the assumed
    // signatures and the integrator adds the
    // `sprites_panel` State field.

    #[test]
    fn step_with_nothing_selected_refuses_without_touching_the_project() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        let depth_before = s.core.undo_depth();
        step_selected(&mut s, SpriteField::X, 1);
        assert_eq!(s.core.status, "Select a sprite first.");
        assert_eq!(s.core.undo_depth(), depth_before, "a refusal snapshots nothing");
        assert_eq!(s.core.project.sprites[0].keys.len(), 1, "no key was authored");
    }

    #[test]
    fn add_key_where_one_exists_refuses_before_any_op() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        // The fixture's first key sits at frame 0, and
        // a fresh core's current frame is 0.
        s.sprites_panel.selected = Some(0);
        let depth_before = s.core.undo_depth();
        add_key_selected(&mut s);
        assert_eq!(s.core.status, "There is already a key at frame 1.");
        assert_eq!(s.core.undo_depth(), depth_before);
    }

    #[test]
    fn delete_key_where_none_exists_refuses_before_any_op() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        s.core.current = 3; // the fixture's one key sits at frame 0
        s.sprites_panel.selected = Some(0);
        let depth_before = s.core.undo_depth();
        delete_key_selected(&mut s);
        assert_eq!(s.core.status, "No key at frame 4 to delete.");
        assert_eq!(s.core.undo_depth(), depth_before);
    }

    #[test]
    fn add_from_asset_appends_and_selects_the_new_sprite() {
        let mut s = crate::init();
        s.core = core_with(&["a.png", "b.png"], &[(0, 1)]);
        add_from_asset(&mut s, 1);
        assert_eq!(s.core.project.sprites.len(), 2, "one instance appended");
        assert_eq!(s.core.project.sprites[1].asset, 1);
        assert_eq!(
            s.sprites_panel.selected,
            Some(1),
            "the new sprite is the selected one"
        );
    }

    #[test]
    fn step_selected_upserts_a_key_at_the_current_frame() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 2)]);
        s.core.current = 1; // between the fixture keys at frames 0 and 2
        s.sprites_panel.selected = Some(0);
        let depth_before = s.core.undo_depth();
        step_selected(&mut s, SpriteField::X, 1);
        let inst = &s.core.project.sprites[0];
        assert_eq!(inst.keys.len(), 3, "a key was authored at frame 1");
        let written = exact_key(inst, 1.0).expect("the new key sits at frame 1");
        assert_eq!(written.x, 105.0 + POS_STEP, "the sampled x, stepped once");
        assert_eq!(written.y, 200.0, "the unstepped fields carry the sample");
        assert_eq!(
            s.core.undo_depth(),
            depth_before + 1,
            "one press is exactly one undo step"
        );
        s.core.undo();
        assert_eq!(s.core.project.sprites[0].keys.len(), 2, "one undo removes the key");
    }

    #[test]
    fn step_selected_on_an_exact_key_steps_that_key_in_place() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 2)]);
        s.core.current = 2; // the fixture's second key sits at frame 2, x=110
        s.sprites_panel.selected = Some(0);
        step_selected(&mut s, SpriteField::Rotation, -1);
        let inst = &s.core.project.sprites[0];
        assert_eq!(inst.keys.len(), 2, "no key added — the existing one was stepped");
        assert_eq!(
            exact_key(inst, 2.0).map(|k| k.angle),
            Some(-ROT_STEP_DEG)
        );
    }

    #[test]
    fn delete_key_selected_removes_only_the_current_frames_key() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 2)]);
        s.core.current = 2;
        s.sprites_panel.selected = Some(0);
        delete_key_selected(&mut s);
        let inst = &s.core.project.sprites[0];
        assert_eq!(inst.keys.len(), 1);
        assert_eq!(inst.keys[0].frame, 0.0, "the frame-0 key survives");
    }

    #[test]
    fn delete_sprite_row_keeps_selection_on_the_same_instance() {
        let mut s = crate::init();
        s.core = core_with(&["a.png", "b.png"], &[(0, 1), (1, 1), (0, 1)]);
        s.sprites_panel.selected = Some(2);
        delete_sprite_row(&mut s, 0);
        assert_eq!(s.core.project.sprites.len(), 2);
        assert_eq!(
            s.sprites_panel.selected,
            Some(1),
            "selection followed the sprite it named across the gap"
        );
        delete_sprite_row(&mut s, 1);
        assert_eq!(
            s.sprites_panel.selected, None,
            "deleting the selected sprite clears the selection"
        );
    }

    #[test]
    fn selected_after_reorder_follows_the_move() {
        // The moved instance's selection follows it.
        assert_eq!(selected_after_reorder(Some(0), 0, 2), Some(2));
        assert_eq!(selected_after_reorder(Some(2), 2, 0), Some(0));
        // Instances between the two positions shift one
        // step toward the vacated slot.
        assert_eq!(selected_after_reorder(Some(1), 0, 2), Some(0));
        assert_eq!(selected_after_reorder(Some(2), 0, 2), Some(1));
        assert_eq!(selected_after_reorder(Some(0), 2, 0), Some(1));
        assert_eq!(selected_after_reorder(Some(1), 2, 0), Some(2));
        // Outside the span: unmoved; no selection: none.
        assert_eq!(selected_after_reorder(Some(3), 0, 2), Some(3));
        assert_eq!(selected_after_reorder(Some(0), 1, 2), Some(0));
        assert_eq!(selected_after_reorder(None, 0, 2), None);
        // A same-position "move" is the identity.
        assert_eq!(selected_after_reorder(Some(1), 1, 1), Some(1));
    }

    #[test]
    fn set_asset_selected_repoints_the_selected_sprite_only() {
        let mut s = crate::init();
        s.core = core_with(&["a.png", "b.png"], &[(0, 2), (1, 1)]);
        s.sprites_panel.selected = Some(0);
        set_asset_selected(&mut s, 1);
        assert_eq!(s.core.project.sprites[0].asset, 1);
        assert_eq!(s.core.project.sprites[0].keys.len(), 2, "keys untouched");
        assert_eq!(s.core.project.sprites[1].asset, 1, "the other sprite is untouched");
        assert_eq!(s.core.status, "Sprite 1 now uses asset 2.");
        // With nothing selected the press earns the
        // panel's refusal and changes nothing.
        s.sprites_panel.selected = None;
        set_asset_selected(&mut s, 0);
        assert_eq!(s.core.status, "Select a sprite first.");
        assert_eq!(s.core.project.sprites[0].asset, 1);
    }

    #[test]
    fn move_selected_swaps_neighbours_and_the_selection_follows() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1), (0, 1), (0, 1)]);
        // Tell the three instances apart by their key x.
        for (i, inst) in s.core.project.sprites.iter_mut().enumerate() {
            inst.keys[0].x = (i + 1) as f32 * 10.0;
        }
        s.sprites_panel.selected = Some(0);
        move_selected(&mut s, 1);
        let xs: Vec<f32> = s.core.project.sprites.iter().map(|i| i.keys[0].x).collect();
        assert_eq!(xs, vec![20.0, 10.0, 30.0], "one step toward the front");
        assert_eq!(s.sprites_panel.selected, Some(1), "the selection follows");
        move_selected(&mut s, 1);
        assert_eq!(s.sprites_panel.selected, Some(2));
        // At the front edge a further press is the op's
        // validated no-op: order and selection stand.
        move_selected(&mut s, 1);
        let xs: Vec<f32> = s.core.project.sprites.iter().map(|i| i.keys[0].x).collect();
        assert_eq!(xs, vec![20.0, 30.0, 10.0]);
        assert_eq!(s.sprites_panel.selected, Some(2));
        assert_eq!(s.core.status, "Sprite 3 is already at position 3.");
        // Back again.
        move_selected(&mut s, -1);
        assert_eq!(s.sprites_panel.selected, Some(1));
        // With nothing selected: the refusal, no motion.
        s.sprites_panel.selected = None;
        move_selected(&mut s, 1);
        assert_eq!(s.core.status, "Select a sprite first.");
    }

    #[test]
    fn sprites_panel_builds_in_every_state() {
        let t = rune::tok::theme(true);
        // Empty project: both empty forms build.
        let mut s = crate::init();
        let _ = sprites_panel(&s, &t);
        // Assets but no sprites: the picker builds.
        s.core = core_with(&["a.png"], &[]);
        let _ = sprites_panel(&s, &t);
        // A selected sprite with an exact key at the
        // current frame: full edit area, live Delete key.
        s.core = core_with(&["a.png"], &[(0, 2)]);
        s.sprites_panel.selected = Some(0);
        let _ = sprites_panel(&s, &t);
        // Between keys: sampled readout, inert Delete key.
        s.core.current = 1;
        let _ = sprites_panel(&s, &t);
        // A stale selection builds the unselected form.
        s.sprites_panel.selected = Some(9);
        let _ = sprites_panel(&s, &t);
        // A sprite naming a missing asset builds too.
        s.core = core_with(&["a.png"], &[(7, 1)]);
        s.sprites_panel.selected = Some(0);
        let _ = sprites_panel(&s, &t);
        // At the scale cap the + button renders inert.
        s.core = core_with(&["a.png"], &[(0, 1)]);
        s.core.project.sprites[0].keys[0].scale = SCALE_MAX;
        s.sprites_panel.selected = Some(0);
        let _ = sprites_panel(&s, &t);
    }
}
