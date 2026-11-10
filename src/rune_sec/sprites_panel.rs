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
//! Wave Y (Y14, SpriteLoop GAP P-6 completion) extends
//! the edit area to the key's enrichment fields (Wave
//! X, X9 — `opacity`, `flip_x` / `flip_y`, `scale_y`,
//! `pivot_x` / `pivot_y`), which until now only the
//! MCP `sprite_set_key` handler and the .nodes
//! importer could write. The new steppers, toggles and
//! the follow-`scale` switch all write through the one
//! full-key op, in exactly one wrapper each
//! ([`step_enriched`], [`toggle_flip_selected`],
//! [`toggle_follow_selected`]):
//!
//! - `core.set_sprite_key_full(index: usize, frame:
//!   usize, key: InstanceKey)` — write a COMPLETE key
//!   (all eleven fields), one undo step; refusals (no
//!   such sprite, a non-finite value) spend no
//!   snapshot.
//!
//! The two pre-existing key writes move to the same
//! op over the same [`base_key`]-built keys: the
//! scalar `set_sprite_key` authors enrichment
//! DEFAULTS on a fresh insert, so an Add key (or a
//! transform step) at a keyless frame between two
//! enriched keys used to snap the new key's opacity /
//! flips / pivot to defaults — a visible pop, and not
//! what the MCP write does (it builds from the
//! sampled base too). Writing the built key whole
//! preserves the enrichment by construction: at an
//! exact key the base IS the stored key (the
//! `scale_y` follow sentinel survives verbatim); at a
//! keyless frame the base is the sample, whose
//! effective values the write materialises — the
//! transform fields' own upsert rule, now applied to
//! all eleven. A second readout line,
//! [`enrichment_readout`], names the enrichment the
//! first line never mentioned.
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

/// Opacity stepper step (Wave Y, Y14) — the style
/// inspector's `OPACITY_STEP`, so every opacity
/// stepper in the shell moves in the same increments.
pub const OPACITY_STEP: f32 = 0.05;
/// Pivot stepper step (Wave Y, Y14): the pivot is an
/// image-relative 0..=1 anchor, so the opacity step
/// size is the natural increment here too.
pub const PIVOT_STEP: f32 = 0.05;
/// Smallest pivot a stepper press can write (the
/// image's own edge; an imported anchor outside the
/// image stays legal data — the stepper just cannot
/// author one, and steps an out-of-range stored
/// pivot back inside, the scale stepper's edge).
pub const PIVOT_MIN: f32 = 0.0;
/// Largest pivot a stepper press can write.
pub const PIVOT_MAX: f32 = 1.0;

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

/// Which enrichment field a stepper press steps
/// (Wave Y, Y14 — the P-6 fields `stepped_key`'s
/// four cannot name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnrichField {
    Opacity,
    ScaleY,
    PivotX,
    PivotY,
}

/// Whether `key`'s stored `scale_y` is the
/// follow-`scale` sentinel — `eff_scale_y`'s own
/// test (a stored value that is not > 0.0, NaN
/// included, means the axis follows `scale`). Pure.
pub fn key_follows_scale(key: &InstanceKey) -> bool {
    !(key.scale_y > 0.0)
}

/// `base` with one enrichment field stepped by
/// `dir` (−1/+1) and its frame set to `frame`: the
/// full-key twin of [`stepped_key`], written whole
/// via `set_sprite_key_full` so every other field —
/// enrichment included — carries over untouched.
/// Opacity steps [`OPACITY_STEP`] clamped to the
/// model's 0..=1 range and rounded to whole percents
/// (the style inspector's `opacity_stepped`, so
/// repeated steps never accumulate float dust);
/// ScaleY steps the EFFECTIVE value
/// (`eff_scale_y()`) by [`SCALE_STEP`] clamped to
/// [`SCALE_MIN`]..=[`SCALE_MAX`] and stores the
/// explicit result — stepping a follow-`scale` key
/// materialises the axis at the stepped value; the
/// pivots step [`PIVOT_STEP`] clamped to
/// [`PIVOT_MIN`]..=[`PIVOT_MAX`] and rounded to two
/// decimals like opacity. Pure.
pub fn stepped_enrich_key(
    base: InstanceKey,
    frame: f32,
    field: EnrichField,
    dir: i32,
) -> InstanceKey {
    let mut key = InstanceKey { frame, ..base };
    match field {
        EnrichField::Opacity => {
            let stepped = base.opacity + dir as f32 * OPACITY_STEP;
            key.opacity = (stepped.clamp(0.0, 1.0) * 100.0).round() / 100.0;
        }
        EnrichField::ScaleY => {
            key.scale_y = (base.eff_scale_y() + dir as f32 * SCALE_STEP)
                .clamp(SCALE_MIN, SCALE_MAX);
        }
        EnrichField::PivotX => {
            let stepped = base.pivot_x + dir as f32 * PIVOT_STEP;
            key.pivot_x = (stepped.clamp(PIVOT_MIN, PIVOT_MAX) * 100.0).round() / 100.0;
        }
        EnrichField::PivotY => {
            let stepped = base.pivot_y + dir as f32 * PIVOT_STEP;
            key.pivot_y = (stepped.clamp(PIVOT_MIN, PIVOT_MAX) * 100.0).round() / 100.0;
        }
    }
    key
}

/// Whether one enrichment step from `key` in `dir`
/// cannot change the value it steps, so the button
/// renders inert — [`scale_step_is_noop`]'s rule per
/// field, judged on the value the step acts on:
/// ScaleY is judged on the EFFECTIVE value (a
/// follow-`scale` key whose axis already sits at the
/// clamp the step pushes against cannot move, so
/// the press cannot happen — the sentinel is only
/// ever materialised by a step that moves it). An
/// out-of-range stored value is NOT a no-op in
/// either direction — each step clamps it back
/// inside the range, a real change. Pure.
pub fn enrich_step_is_noop(key: &InstanceKey, field: EnrichField, dir: i32) -> bool {
    let stepped = stepped_enrich_key(*key, key.frame, field, dir);
    match field {
        EnrichField::Opacity => stepped.opacity == key.opacity,
        EnrichField::ScaleY => stepped.eff_scale_y() == key.eff_scale_y(),
        EnrichField::PivotX => stepped.pivot_x == key.pivot_x,
        EnrichField::PivotY => stepped.pivot_y == key.pivot_y,
    }
}

/// Which axis a flip toggle flips (Wave Y, Y14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipAxis {
    X,
    Y,
}

/// `base` with one flip axis toggled and its frame
/// set to `frame`: the other axis and every other
/// field carry over untouched. Pure.
pub fn flipped_key(base: InstanceKey, frame: f32, axis: FlipAxis) -> InstanceKey {
    let mut key = InstanceKey { frame, ..base };
    match axis {
        FlipAxis::X => key.flip_x = !base.flip_x,
        FlipAxis::Y => key.flip_y = !base.flip_y,
    }
    key
}

/// `base` with its follow-`scale` state toggled and
/// its frame set to `frame`: a following key stores
/// its current effective `scale_y` explicitly (the
/// axis freezes at the value it draws today); an
/// explicit key stores the 0.0 sentinel (the axis
/// rejoins `scale`). Every other field carries over
/// untouched. Pure.
pub fn follow_toggled_key(base: InstanceKey, frame: f32) -> InstanceKey {
    let mut key = InstanceKey { frame, ..base };
    key.scale_y = if key_follows_scale(&base) {
        base.eff_scale_y()
    } else {
        0.0
    };
    key
}

/// % readout for an opacity value (the style
/// inspector's `opacity_text` shape: `100%`, `55%`,
/// `5%`). Pure.
pub fn opacity_value(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i32)
}

/// The edit area's enrichment readout line for
/// `key` (Wave Y, Y14): the P-6 fields
/// [`transform_readout`] never named, in stepper
/// order — opacity as a percent, the flips, the
/// EFFECTIVE scale_y (with "(follows Scale)" exactly
/// while the stored field is the sentinel), and the
/// pivot. Pure — the builder and the dopesheet lane
/// render this string verbatim, so the wording is
/// pinned here, once, for both surfaces.
pub fn enrichment_readout(key: &InstanceKey) -> String {
    let flip = match (key.flip_x, key.flip_y) {
        (true, true) => "X+Y",
        (true, false) => "X",
        (false, true) => "Y",
        (false, false) => "none",
    };
    let scale_y = if key_follows_scale(key) {
        format!("×{:.2} (follows Scale)", key.eff_scale_y())
    } else {
        format!("×{:.2}", key.eff_scale_y())
    };
    format!(
        "Opacity {} · Flip {} · Scale Y {} · Pivot {:.2}, {:.2}",
        opacity_value(key.opacity),
        flip,
        scale_y,
        key.pivot_x,
        key.pivot_y
    )
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
/// the full-key upsert (`set_sprite_key_full` — Wave
/// Y, Y14; see the module docs for why the panel
/// writes whole keys). The value written is
/// [`stepped_key`] over [`base_key`], so a press at
/// a keyless frame authors the key there, carrying
/// the sampled enrichment instead of snapping it to
/// defaults. A stale selection refuses here ("No
/// such sprite.") before any op is called.
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
    s.core.set_sprite_key_full(i, key.frame as usize, key);
}

/// Enrichment stepper handler (Wave Y, Y14): step
/// one P-6 field of the selected instance's key at
/// the current frame, via `set_sprite_key_full`.
/// The value written is [`stepped_enrich_key`] over
/// [`base_key`] — the whole key, so the transform
/// and the unstepped enrichment carry over exactly.
/// Same refusal shape as [`step_selected`].
fn step_enriched(s: &mut State, field: EnrichField, dir: i32) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i).cloned() else {
        s.core.status = "No such sprite.".into();
        return;
    };
    let key = stepped_enrich_key(base_key(&s.core, &inst, frame), frame, field, dir);
    s.core.set_sprite_key_full(i, key.frame as usize, key);
}

/// Flip toggle handler (Wave Y, Y14): toggle one
/// flip axis of the selected instance's key at the
/// current frame, via `set_sprite_key_full` over
/// [`flipped_key`] — one undo step, the other axis
/// and every other field untouched. Same refusal
/// shape as [`step_selected`].
fn toggle_flip_selected(s: &mut State, axis: FlipAxis) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i).cloned() else {
        s.core.status = "No such sprite.".into();
        return;
    };
    let key = flipped_key(base_key(&s.core, &inst, frame), frame, axis);
    s.core.set_sprite_key_full(i, key.frame as usize, key);
}

/// Follow-`scale` toggle handler (Wave Y, Y14):
/// switch the selected instance's key at the
/// current frame between an explicit `scale_y` and
/// the follow sentinel, via `set_sprite_key_full`
/// over [`follow_toggled_key`] — one undo step.
/// Same refusal shape as [`step_selected`].
fn toggle_follow_selected(s: &mut State) {
    let Some(i) = selected_index(s) else {
        return;
    };
    let frame = current_frame(&s.core);
    let Some(inst) = s.core.project.sprites.get(i).cloned() else {
        s.core.status = "No such sprite.".into();
        return;
    };
    let key = follow_toggled_key(base_key(&s.core, &inst, frame), frame);
    s.core.set_sprite_key_full(i, key.frame as usize, key);
}

/// "Add key" handler: author a key at the current
/// frame holding the sampled transform unchanged —
/// all eleven fields of it (Wave Y, Y14: the write
/// is the full-key upsert over [`base_key`], so the
/// sampled enrichment is authored too, not the
/// defaults the scalar op inserts). When a key
/// already exists at this frame the press refuses
/// here — the sampled value IS that key, so the
/// write would only mint a phantom undo step.
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
    s.core.set_sprite_key_full(i, key.frame as usize, key);
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

/// One enrichment stepper row (Wave Y, Y14):
/// [`stepper_row`]'s shape — label, − button, value
/// readout, + button — over an [`EnrichField`],
/// pressing through [`step_enriched`].
fn enrich_stepper_row(
    row_key: &str,
    label: &str,
    dn_key: &str,
    dn_label: &str,
    up_key: &str,
    up_label: &str,
    value: &str,
    field: EnrichField,
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
            step_enriched(s, field, -1)
        }))
        .child(design::stepper_value(value, t))
        .child(step_btn(up_key, up_label, live, noop_up, t, move |s: &mut State| {
            step_enriched(s, field, 1)
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
        // The enrichment line (Wave Y, Y14): the P-6
        // fields the transform line never named, in
        // the one wording both this panel and the
        // dopesheet lane render.
        edit = edit.child(
            text(enrichment_readout(&key))
                .key("sprite-enrich-readout")
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

    // The enrichment rows' values and clamp states
    // (Wave Y, Y14): Scale Y reads its EFFECTIVE
    // value (what the passes draw), the flips and
    // the follow state ride the toggle buttons'
    // selected marks below.
    let (opacity_val, scaley_val, pivotx_val, pivoty_val) = match base {
        Some(k) => (
            opacity_value(k.opacity),
            format!("×{:.2}", k.eff_scale_y()),
            format!("{:.2}", k.pivot_x),
            format!("{:.2}", k.pivot_y),
        ),
        None => (
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
        ),
    };
    let opacity_noop_dn =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::Opacity, -1));
    let opacity_noop_up =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::Opacity, 1));
    let scaley_noop_dn =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::ScaleY, -1));
    let scaley_noop_up =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::ScaleY, 1));
    let pivotx_noop_dn =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::PivotX, -1));
    let pivotx_noop_up =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::PivotX, 1));
    let pivoty_noop_dn =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::PivotY, -1));
    let pivoty_noop_up =
        base.map_or(false, |k| enrich_step_is_noop(&k, EnrichField::PivotY, 1));
    let (flip_x_on, flip_y_on) = match base {
        Some(k) => (k.flip_x, k.flip_y),
        None => (false, false),
    };
    let following = base.map_or(false, |k| key_follows_scale(&k));

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
    // The row wraps (Wave Y, Y14 — an overflow repair,
    // Y15's citation audit): on one line the four
    // children measure 330 pt against the card's
    // 316 pt content width (tree-default.json on the
    // Wave X integration dispatch: label 64 + "Send
    // backward" 109.5 + value 34 + "Bring forward"
    // 98.5 + 3 gaps of 8), so `sprite-z-front` ended
    // at right edge 1426.0 — +14.0 pt past the 1412.5
    // edge, its inner element +4.0 — in every dump
    // whose edit area renders (40 scan hits, one
    // family). The row already carries `.w_full()`,
    // the bounded width wrap needs to engage (the
    // O-1 lesson), so `.flex_wrap()` moves "Bring
    // forward" to a second line at the row's left
    // (right edge 1096 + 98.5 = 1194.5, inside);
    // keys, labels, handlers and the inert-at-edge
    // discipline are unchanged.
    let z_row = row()
        .key("sprite-z-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .flex_wrap()
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

    // The follow-Scale toggle (Wave Y, Y14): one
    // button whose selected mark IS the base key's
    // follow state (the asset picker's current-asset
    // idiom); a press stores the other state — the
    // sentinel, or today's effective value explicitly.
    // Muted-live with nothing selected, like every
    // edit-area control.
    let follow_row = row()
        .key("sprite-follow-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(if edit_live {
            btn(
                "sprite-scaley-follow",
                "Scale Y follows Scale",
                following,
                t,
                |s: &mut State| toggle_follow_selected(s),
            )
        } else {
            btn_muted_live(
                "sprite-scaley-follow",
                "Scale Y follows Scale",
                false,
                t,
                |s: &mut State| toggle_follow_selected(s),
            )
        });

    // The flip toggles (Wave Y, Y14): one button per
    // axis, each carrying the base key's flip on that
    // axis as its selected mark; a press toggles that
    // axis only. Muted-live with nothing selected.
    let flip_row = row()
        .key("sprite-flip-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("Flip")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(if edit_live {
            btn(
                "sprite-flip-x",
                "Flip X",
                flip_x_on,
                t,
                |s: &mut State| toggle_flip_selected(s, FlipAxis::X),
            )
        } else {
            btn_muted_live(
                "sprite-flip-x",
                "Flip X",
                false,
                t,
                |s: &mut State| toggle_flip_selected(s, FlipAxis::X),
            )
        })
        .child(if edit_live {
            btn(
                "sprite-flip-y",
                "Flip Y",
                flip_y_on,
                t,
                |s: &mut State| toggle_flip_selected(s, FlipAxis::Y),
            )
        } else {
            btn_muted_live(
                "sprite-flip-y",
                "Flip Y",
                false,
                t,
                |s: &mut State| toggle_flip_selected(s, FlipAxis::Y),
            )
        });

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
        .child(enrich_stepper_row(
            "sprite-opacity-row", "Opacity",
            "sprite-opacity-dn", "Opacity −", "sprite-opacity-up", "Opacity +",
            &opacity_val, EnrichField::Opacity, edit_live, opacity_noop_dn, opacity_noop_up, t,
        ))
        .child(enrich_stepper_row(
            "sprite-scaley-row", "Scale Y",
            "sprite-scaley-dn", "Scale Y −", "sprite-scaley-up", "Scale Y +",
            &scaley_val, EnrichField::ScaleY, edit_live, scaley_noop_dn, scaley_noop_up, t,
        ))
        .child(follow_row)
        .child(flip_row)
        .child(enrich_stepper_row(
            "sprite-pivotx-row", "Pivot X",
            "sprite-pivotx-dn", "Pivot X −", "sprite-pivotx-up", "Pivot X +",
            &pivotx_val, EnrichField::PivotX, edit_live, pivotx_noop_dn, pivotx_noop_up, t,
        ))
        .child(enrich_stepper_row(
            "sprite-pivoty-row", "Pivot Y",
            "sprite-pivoty-dn", "Pivot Y −", "sprite-pivoty-up", "Pivot Y +",
            &pivoty_val, EnrichField::PivotY, edit_live, pivoty_noop_dn, pivoty_noop_up, t,
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

    /// Enrichment-field equality for a key (Wave Y,
    /// Y14): the six P-6 fields [`assert_key`] does
    /// not compare.
    fn assert_enrichment(
        actual: InstanceKey,
        opacity: f32,
        flip_x: bool,
        flip_y: bool,
        scale_y: f32,
        pivot_x: f32,
        pivot_y: f32,
    ) {
        assert_eq!(actual.opacity, opacity, "opacity");
        assert_eq!(actual.flip_x, flip_x, "flip_x");
        assert_eq!(actual.flip_y, flip_y, "flip_y");
        assert_eq!(actual.scale_y, scale_y, "scale_y");
        assert_eq!(actual.pivot_x, pivot_x, "pivot_x");
        assert_eq!(actual.pivot_y, pivot_y, "pivot_y");
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
    fn stepped_enrich_key_steps_each_field_and_reframes() {
        // Wave Y (Y14): the enrichment stepper math.
        let mut base = key(0.0, 100.0, 200.0, 2.0, 30.0);
        base.opacity = 0.5;
        base.scale_y = 1.0;
        base.pivot_x = 0.25;
        base.pivot_y = 0.75;
        let stepped = stepped_enrich_key(base, 5.0, EnrichField::Opacity, 1);
        assert_eq!(stepped.frame, 5.0, "the write is re-framed");
        assert_eq!(stepped.opacity, 0.55);
        assert_enrichment(stepped, 0.55, false, false, 1.0, 0.25, 0.75);
        assert_key(stepped, key(5.0, 100.0, 200.0, 2.0, 30.0));
        assert_eq!(
            stepped_enrich_key(base, 5.0, EnrichField::ScaleY, -1).scale_y,
            0.9,
            "an explicit scale_y steps its own value"
        );
        assert_eq!(
            stepped_enrich_key(base, 5.0, EnrichField::PivotX, 1).pivot_x,
            0.3
        );
        assert_eq!(
            stepped_enrich_key(base, 5.0, EnrichField::PivotY, -1).pivot_y,
            0.7
        );
        // Repeated opacity steps land on whole
        // percents, never float dust.
        let mut k = base;
        for _ in 0..4 {
            k = stepped_enrich_key(k, 0.0, EnrichField::Opacity, 1);
        }
        assert_eq!(k.opacity, 0.7);
    }

    #[test]
    fn stepped_enrich_key_materialises_a_following_scale_y_only_when_it_moves() {
        // A follow-`scale` key (sentinel 0.0, scale
        // 2.0): stepping Scale Y stores the stepped
        // EFFECTIVE value explicitly.
        let base = key(0.0, 0.0, 0.0, 2.0, 0.0);
        assert!(key_follows_scale(&base));
        assert_eq!(
            stepped_enrich_key(base, 0.0, EnrichField::ScaleY, 1).scale_y,
            2.1
        );
        assert_eq!(
            stepped_enrich_key(base, 0.0, EnrichField::ScaleY, -1).scale_y,
            1.9
        );
        // At the effective cap the step cannot move
        // the axis, so it is a no-op and the button
        // renders inert — the sentinel is NOT silently
        // materialised by a press that changes nothing.
        let capped = key(0.0, 0.0, 0.0, SCALE_MAX, 0.0);
        assert!(enrich_step_is_noop(&capped, EnrichField::ScaleY, 1));
        assert!(!enrich_step_is_noop(&capped, EnrichField::ScaleY, -1));
    }

    #[test]
    fn enrich_step_is_noop_only_at_the_clamp_it_pushes_against() {
        let mut k = key(0.0, 0.0, 0.0, 1.0, 0.0);
        k.opacity = 1.0;
        assert!(enrich_step_is_noop(&k, EnrichField::Opacity, 1));
        assert!(!enrich_step_is_noop(&k, EnrichField::Opacity, -1));
        k.opacity = 0.0;
        assert!(enrich_step_is_noop(&k, EnrichField::Opacity, -1));
        k.pivot_x = 0.0;
        assert!(enrich_step_is_noop(&k, EnrichField::PivotX, -1));
        assert!(!enrich_step_is_noop(&k, EnrichField::PivotX, 1));
        k.pivot_y = 1.0;
        assert!(enrich_step_is_noop(&k, EnrichField::PivotY, 1));
        // An out-of-range stored pivot (the MCP write
        // allows any finite anchor) steps back inside
        // — a real change, never a no-op.
        k.pivot_y = 1.5;
        assert!(!enrich_step_is_noop(&k, EnrichField::PivotY, -1));
        assert_eq!(
            stepped_enrich_key(k, 0.0, EnrichField::PivotY, -1).pivot_y,
            PIVOT_MAX
        );
    }

    #[test]
    fn flipped_key_toggles_one_axis_only() {
        let mut base = key(0.0, 100.0, 200.0, 1.0, 30.0);
        base.opacity = 0.5;
        let fx = flipped_key(base, 3.0, FlipAxis::X);
        assert_enrichment(fx, 0.5, true, false, 0.0, 0.5, 0.5);
        assert_eq!(fx.frame, 3.0);
        let fy = flipped_key(fx, 3.0, FlipAxis::Y);
        assert_enrichment(fy, 0.5, true, true, 0.0, 0.5, 0.5);
        let back = flipped_key(fy, 3.0, FlipAxis::X);
        assert_enrichment(back, 0.5, false, true, 0.0, 0.5, 0.5);
    }

    #[test]
    fn follow_toggled_key_swaps_the_sentinel_for_the_effective_value() {
        // Following (scale 2.0, sentinel): the toggle
        // freezes the axis at today's effective value.
        let following = key(0.0, 0.0, 0.0, 2.0, 0.0);
        let frozen = follow_toggled_key(following, 1.0);
        assert_eq!(frozen.scale_y, 2.0);
        assert!(!key_follows_scale(&frozen));
        // Explicit: the toggle rejoins `scale`.
        let rejoined = follow_toggled_key(frozen, 1.0);
        assert_eq!(rejoined.scale_y, 0.0);
        assert!(key_follows_scale(&rejoined));
        assert_eq!(rejoined.frame, 1.0);
    }

    #[test]
    fn enrichment_readout_names_every_p6_field() {
        let plain = key(2.0, 480.0, 360.0, 1.5, 45.0);
        assert_eq!(
            enrichment_readout(&plain),
            "Opacity 100% · Flip none · Scale Y ×1.50 (follows Scale) · Pivot 0.50, 0.50"
        );
        let mut rich = key(2.0, 480.0, 360.0, 2.0, 45.0);
        rich.opacity = 0.5;
        rich.flip_x = true;
        rich.scale_y = 1.0;
        rich.pivot_x = 0.25;
        rich.pivot_y = 0.75;
        assert_eq!(
            enrichment_readout(&rich),
            "Opacity 50% · Flip X · Scale Y ×1.00 · Pivot 0.25, 0.75"
        );
        rich.flip_y = true;
        assert!(enrichment_readout(&rich).contains("Flip X+Y"));
        rich.flip_x = false;
        assert!(enrichment_readout(&rich).contains("Flip Y ·"));
    }

    #[test]
    fn opacity_value_is_a_whole_percent() {
        assert_eq!(opacity_value(1.0), "100%");
        assert_eq!(opacity_value(0.55), "55%");
        assert_eq!(opacity_value(0.05), "5%");
        assert_eq!(opacity_value(0.0), "0%");
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
    fn step_enriched_writes_the_whole_key_in_one_undo_step() {
        // Wave Y (Y14): an enrichment step on an exact
        // enriched key steps its field and preserves
        // every other field, enrichment included.
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        {
            let k = &mut s.core.project.sprites[0].keys[0];
            k.opacity = 0.5;
            k.flip_y = true;
            k.scale_y = 2.0;
            k.pivot_x = 0.25;
            k.pivot_y = 0.75;
        }
        s.sprites_panel.selected = Some(0);
        let depth_before = s.core.undo_depth();
        step_enriched(&mut s, EnrichField::Opacity, 1);
        let written = exact_key(&s.core.project.sprites[0], 0.0).expect("the key");
        assert_enrichment(written, 0.55, false, true, 2.0, 0.25, 0.75);
        assert_eq!(
            s.core.undo_depth(),
            depth_before + 1,
            "one press is exactly one undo step"
        );
        s.core.undo();
        let restored = exact_key(&s.core.project.sprites[0], 0.0).expect("the key");
        assert_eq!(restored.opacity, 0.5, "one undo restores the pre-step key");
    }

    #[test]
    fn enrichment_handlers_refuse_with_nothing_selected() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        let depth_before = s.core.undo_depth();
        step_enriched(&mut s, EnrichField::PivotX, 1);
        assert_eq!(s.core.status, "Select a sprite first.");
        toggle_flip_selected(&mut s, FlipAxis::X);
        assert_eq!(s.core.status, "Select a sprite first.");
        toggle_follow_selected(&mut s);
        assert_eq!(s.core.status, "Select a sprite first.");
        assert_eq!(s.core.undo_depth(), depth_before, "refusals snapshot nothing");
        assert_enrichment(
            s.core.project.sprites[0].keys[0],
            1.0, false, false, 0.0, 0.5, 0.5,
        );
    }

    #[test]
    fn toggle_flip_selected_flips_one_axis_per_press() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        s.sprites_panel.selected = Some(0);
        let depth_before = s.core.undo_depth();
        toggle_flip_selected(&mut s, FlipAxis::X);
        assert_enrichment(
            exact_key(&s.core.project.sprites[0], 0.0).expect("the key"),
            1.0, true, false, 0.0, 0.5, 0.5,
        );
        assert_eq!(s.core.undo_depth(), depth_before + 1);
        toggle_flip_selected(&mut s, FlipAxis::X);
        assert_enrichment(
            exact_key(&s.core.project.sprites[0], 0.0).expect("the key"),
            1.0, false, false, 0.0, 0.5, 0.5,
        );
    }

    #[test]
    fn toggle_follow_selected_authors_and_clears_the_sentinel() {
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 1)]);
        // The fixture key follows its scale (1.0).
        s.sprites_panel.selected = Some(0);
        toggle_follow_selected(&mut s);
        let frozen = exact_key(&s.core.project.sprites[0], 0.0).expect("the key");
        assert_eq!(frozen.scale_y, 1.0, "the axis froze at its effective value");
        toggle_follow_selected(&mut s);
        let rejoined = exact_key(&s.core.project.sprites[0], 0.0).expect("the key");
        assert_eq!(rejoined.scale_y, 0.0, "the axis follows scale again");
    }

    #[test]
    fn add_key_between_enriched_keys_authors_the_sampled_enrichment() {
        // Wave Y (Y14): the Add-key wart — the scalar
        // op used to author enrichment DEFAULTS on a
        // fresh insert, so the sprite popped at the
        // authored frame. The full write authors the
        // sample whole.
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 2)]);
        for k in &mut s.core.project.sprites[0].keys {
            k.opacity = 0.5;
            k.flip_x = true;
            k.pivot_x = 0.25;
            k.pivot_y = 0.75;
        }
        s.core.current = 1; // between the fixture keys at frames 0 and 2
        s.sprites_panel.selected = Some(0);
        add_key_selected(&mut s);
        let written = exact_key(&s.core.project.sprites[0], 1.0).expect("the new key");
        // (scale_y: both neighbours follow their
        // scale 1.0, and a sample interpolates the
        // EFFECTIVE values — the authored key stores
        // the explicit 1.0 it sampled, the documented
        // materialisation.)
        assert_enrichment(written, 0.5, true, false, 1.0, 0.25, 0.75);
    }

    #[test]
    fn step_at_a_keyless_frame_between_enriched_keys_keeps_the_sample() {
        // The transform steppers' twin wart, same fix:
        // stepping X between two enriched keys must
        // not snap the inserted key's enrichment to
        // defaults.
        let mut s = crate::init();
        s.core = core_with(&["a.png"], &[(0, 2)]);
        for k in &mut s.core.project.sprites[0].keys {
            k.opacity = 0.5;
            k.flip_x = true;
        }
        s.core.current = 1;
        s.sprites_panel.selected = Some(0);
        step_selected(&mut s, SpriteField::X, 1);
        let written = exact_key(&s.core.project.sprites[0], 1.0).expect("the new key");
        assert_eq!(written.x, 105.0 + POS_STEP);
        // (scale_y materialises the sampled effective
        // 1.0, as in the Add-key pin above.)
        assert_enrichment(written, 0.5, true, false, 1.0, 0.5, 0.5);
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
        // An enriched key (Wave Y, Y14): the full edit
        // area builds — opacity at its floor renders
        // its − button inert, the flip and follow
        // toggles carry their marks.
        s.core = core_with(&["a.png"], &[(0, 1)]);
        {
            let k = &mut s.core.project.sprites[0].keys[0];
            k.opacity = 0.0;
            k.flip_x = true;
            k.scale_y = 2.0;
            k.pivot_x = 0.25;
        }
        s.sprites_panel.selected = Some(0);
        let _ = sprites_panel(&s, &t);
    }
}
