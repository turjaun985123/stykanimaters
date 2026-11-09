//! Text + sprite segment rendering.
//!
//! A node may carry [`SegmentMedia`](crate::model::SegmentMedia):
//! an image from the project's asset table, or a text label. This
//! module turns those into concrete draw items for a frame
//! ([`media_items`]) and rasterises them into a rendered frame
//! ([`composite_media`]).
//!
//! Geometry: an image becomes a quad centred on the node's segment
//! (parent joint -> node joint). The quad's long axis follows the
//! segment, its length is the segment length, and its height is
//! `length / aspect`, where the aspect is the decoded PNG's
//! width / height, so sprites are never stretched. Text sits at the
//! segment midpoint, rotated along the segment direction.
//!
//! Free-standing instances (`Project.sprites` / `Project.texts`,
//! keyframed with `InstanceKey`s — e.g. imported from Stick Nodes
//! animation files) are placed by the same [`media_items`] pass,
//! after the segment media, so instances draw on top. A sprite
//! instance's `InstanceKey` gives its centre, rotation and scale:
//! the drawn quad is the image's pixel size times the key scale,
//! rotated about the centre. A text instance draws with glyph
//! height `size * key.scale` rotated by the key angle.
//!
//! Text is drawn with a built-in 3x5 bitmap font — no font files, no
//! platform dependencies, fully deterministic. The charset is
//! deliberately small: `A-Z` (lowercase is folded to uppercase),
//! `0-9`, space, and `. , ! ? ' " - : ; ( ) + / & % * = < > _ # $ @`.
//! Any other character renders as `?`.
//!
//! Conventions and limits:
//! - Hidden figures contribute no media (same as the rasterizer).
//! - A node with no parent has no segment; its media is placed at
//!   the joint, extending `length` units along +x. (Media normally
//!   belongs on real segments.)
//! - An image whose asset index is out of range, or whose PNG
//!   bytes fail to decode, is skipped silently.
//! - PNG assets are decoded on each call (no cache): fine for
//!   export-time compositing, not a per-frame editor hot path.
//!
//! The asset table's MCP handlers also live in this module
//! (`tool_assets_list`, `tool_asset_info`, `tool_asset_import`,
//! `tool_asset_rename`, `tool_asset_remove`, `tool_asset_attach`):
//! import validates PNG magic bytes and decodability and caps
//! the stored size; removal refuses while anything references
//! the asset, and when it does remove, references to higher
//! indices are remapped down by one inside the same undo step.

use crate::editor::EditorCore;
use crate::model::{evaluate, ImageAsset, InstanceKey, Project, SegmentMedia, V2};
use image::{Rgba, RgbaImage};
use serde_json::{json, Value};

/// One piece of segment media placed in stage space for a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaItem {
    /// A sprite quad. Corners are ordered so the quad is
    /// `c0 + u*(c1-c0) + v*(c3-c0)` for u, v in [0, 1]: c0/c1 are
    /// the parent-end/node-end corners on one side, c3 mirrors c0.
    /// `alpha` is the opacity the image blends at (Wave 12, A6
    /// §3a): segment media carry figure opacity × node style
    /// opacity — the same product the segment itself draws
    /// with — where before this field existed the compositor
    /// ignored both and drew every image at full strength;
    /// free-standing sprite instances carry 1.0 (an instance
    /// key has no opacity channel).
    Image { corners: [V2; 4], asset: usize, alpha: f32 },
    /// A text label centred at `pos`, running along `angle_deg`
    /// (stage-space degrees, y-down like the rest of the model).
    /// `size` is the glyph height in stage units.
    Text {
        pos: V2,
        angle_deg: f32,
        content: String,
        size: f32,
        color: u32,
    },
}

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: V2, s: f32) -> V2 {
    [a[0] * s, a[1] * s]
}

/// Pixel dimensions of asset `idx`, or None when it is missing
/// or its PNG bytes do not decode.
pub fn png_dims(project: &Project, idx: usize) -> Option<(u32, u32)> {
    let asset = project.assets.get(idx)?;
    let img = image::load_from_memory(&asset.png).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Width / height of asset `idx`, or None when it is missing or
/// its PNG bytes do not decode.
fn png_aspect(project: &Project, idx: usize) -> Option<f32> {
    let (w, h) = png_dims(project, idx)?;
    Some(w as f32 / h as f32)
}

/// All media items visible at fractional frame time `frame_f`:
/// segment media in figure order, then node order; then sprite
/// instances; then text instances (instances draw on top).
pub fn media_items(project: &Project, frame_f: f32) -> Vec<MediaItem> {
    let mut out = Vec::new();
    let poses = project.poses_at(frame_f);
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let Some(pose) = poses.get(fi) else { continue };
        let pos = evaluate(&fig.def, pose);
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            let Some(media) = &node.media else { continue };
            let (a, b) = match node.parent {
                Some(p) => (pos[p], pos[ni]),
                None => (pos[ni], add(pos[ni], [node.length, 0.0])),
            };
            let ab = sub(b, a);
            let len = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
            if len <= f32::EPSILON {
                continue;
            }
            let d = scale(ab, 1.0 / len);
            let mid = scale(add(a, b), 0.5);
            match media {
                SegmentMedia::Image { asset } => {
                    let Some(aspect) = png_aspect(project, *asset) else {
                        continue;
                    };
                    let n = [-d[1], d[0]];
                    let hl = scale(d, len / 2.0);
                    let hh = scale(n, len / aspect / 2.0);
                    out.push(MediaItem::Image {
                        corners: [
                            sub(sub(mid, hl), hh),
                            sub(add(mid, hl), hh),
                            add(add(mid, hl), hh),
                            add(sub(mid, hl), hh),
                        ],
                        asset: *asset,
                        alpha: fig.opacity.clamp(0.0, 1.0)
                            * node.style.opacity.clamp(0.0, 1.0),
                    });
                }
                SegmentMedia::Text { content, size } => {
                    out.push(MediaItem::Text {
                        pos: mid,
                        angle_deg: d[1].atan2(d[0]).to_degrees(),
                        content: content.clone(),
                        size: *size,
                        color: node.color.unwrap_or(fig.color),
                    });
                }
            }
        }
    }
    // Free-standing sprite instances: the image's pixel size times
    // the sampled key scale, centred at the key position, rotated
    // by the key angle (y-down rotation, as for text).
    for inst in &project.sprites {
        let Some(key) = InstanceKey::sample(&inst.keys, frame_f) else {
            continue;
        };
        if key.scale <= 0.0 {
            continue;
        }
        let Some((w, h)) = png_dims(project, inst.asset) else {
            continue;
        };
        let c = [key.x, key.y];
        let rad = key.angle.to_radians();
        let u = [rad.cos(), rad.sin()];
        let v = [-rad.sin(), rad.cos()];
        let hu = scale(u, w as f32 * key.scale / 2.0);
        let hv = scale(v, h as f32 * key.scale / 2.0);
        out.push(MediaItem::Image {
            corners: [
                sub(sub(c, hu), hv),
                sub(add(c, hu), hv),
                add(add(c, hu), hv),
                add(sub(c, hu), hv),
            ],
            asset: inst.asset,
            alpha: 1.0,
        });
    }
    // Free-standing text instances: `size` is the unscaled glyph
    // height; the key scale multiplies it.
    for text in &project.texts {
        let Some(key) = InstanceKey::sample(&text.keys, frame_f) else {
            continue;
        };
        let size = text.size * key.scale;
        if size <= 0.0 {
            continue;
        }
        out.push(MediaItem::Text {
            pos: [key.x, key.y],
            angle_deg: key.angle,
            content: text.content.clone(),
            size,
            color: text.color,
        });
    }
    out
}

/// Draw every media item for `frame_f` into `img`. `view` maps
/// stage coordinates to image pixel coordinates (the same closure
/// shape the export rasterizer builds). Images are sampled
/// nearest-neighbour through the quad's affine map and alpha-blended
/// over the destination; text is stamped as solid glyph blocks.
pub fn composite_media(
    project: &Project,
    frame_f: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    // Stage-units -> pixels scale, probed from the view: the
    // length of the mapped x basis vector. Views may also rotate
    // (camera angle, Wave 6) — a basis vector's length is the
    // uniform scale with or without rotation, so this probe
    // stays exact.
    let o = view([0.0, 0.0]);
    let ex = view([1.0, 0.0]);
    let view_scale = ((ex[0] - o[0]).powi(2) + (ex[1] - o[1]).powi(2)).sqrt();
    let view_scale = if view_scale > f32::EPSILON {
        view_scale
    } else {
        1.0
    };

    for item in media_items(project, frame_f) {
        match item {
            MediaItem::Image { corners, asset, alpha } => {
                composite_image(project, asset, corners, alpha, img, view);
            }
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
            } => composite_text(pos, angle_deg, &content, size, color, img, view, view_scale),
        }
    }
}

// ---------------------------------------------------------------------
// Asset-table MCP handlers (Wave 12)
//
// `Project.assets` had no MCP surface before this wave: the only
// way an asset entered a project through a tool was
// `attach_image_to_node`, whose core op reads the first PNG in
// ./import and attaches it in the same breath — there was no way
// to list, inspect, rename, remove or deliberately attach an
// asset. These handlers give the table its full lifecycle. The
// reference census below is the whole set of places an asset
// index can live: segment media on figure defs, segment media
// on movieclip defs (`ClipDef` carries no asset table of its
// own, so its nodes' indices name project assets), and
// free-standing sprite instances.
// ---------------------------------------------------------------------

/// PNG signature every imported asset must start with.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Largest file `tool_asset_import` will store, in bytes
/// (8 MiB). Asset bytes are stored verbatim inside the project
/// JSON (as a number array) and inside every undo snapshot, so
/// an unbounded import would bloat each save and each undo
/// step several times over.
pub const ASSET_IMPORT_MAX_BYTES: u64 = 8 * 1024 * 1024;

fn ast_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.as_object().and_then(|o| o.get(key))
}

fn ast_as_usize(v: &Value, key: &str) -> Result<usize, String> {
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

fn ast_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match ast_get(args, key) {
        Some(v) => ast_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn ast_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match ast_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn ast_opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match ast_get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

fn ast_check_asset(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.assets.len();
    if index >= n {
        return Err(format!(
            "asset index {index} out of range (project has {n} asset(s))"
        ));
    }
    Ok(())
}

fn ast_check_node(core: &EditorCore, fi: usize, ni: usize) -> Result<(), String> {
    let figures = core.project.figures.len();
    if fi >= figures {
        return Err(format!(
            "figure index {fi} out of range (project has {figures} figure(s))"
        ));
    }
    let n = core.project.figures[fi].def.nodes.len();
    if ni >= n {
        return Err(format!(
            "node index {ni} out of range (figure {fi} has {n} node(s))"
        ));
    }
    Ok(())
}

/// Pixel dimensions of one asset's bytes, or None when they do
/// not decode (the same tolerance `png_dims` applies at render
/// time, reported here instead of skipped silently).
fn asset_dims(asset: &ImageAsset) -> Option<(u32, u32)> {
    let img = image::load_from_memory(&asset.png).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Count every reference to asset `asset`, as
/// (figure segment images, clip-def segment images, sprite
/// instances). This is the complete census: an asset index can
/// only be stored in those three places.
fn asset_references(project: &Project, asset: usize) -> (usize, usize, usize) {
    let mut segments = 0;
    for fig in &project.figures {
        for node in &fig.def.nodes {
            if matches!(&node.media, Some(SegmentMedia::Image { asset: a }) if *a == asset) {
                segments += 1;
            }
        }
    }
    let mut clip_segments = 0;
    for clip in &project.clips {
        for node in &clip.def.nodes {
            if matches!(&node.media, Some(SegmentMedia::Image { asset: a }) if *a == asset) {
                clip_segments += 1;
            }
        }
    }
    let sprites = project.sprites.iter().filter(|s| s.asset == asset).count();
    (segments, clip_segments, sprites)
}

/// After removing asset `removed` from a table that had
/// `old_len` entries, shift every still-valid reference above
/// `removed` down by one, across all three reference kinds;
/// returns how many references moved. References that were
/// already dangling (>= `old_len`) are left untouched — they
/// named nothing before the removal either, and the renderers
/// skip them.
fn remap_asset_refs(project: &mut Project, removed: usize, old_len: usize) -> usize {
    fn shift(media: &mut Option<SegmentMedia>, removed: usize, old_len: usize) -> bool {
        match media {
            Some(SegmentMedia::Image { asset }) if *asset > removed && *asset < old_len => {
                *asset -= 1;
                true
            }
            _ => false,
        }
    }
    let mut moved = 0;
    for fig in &mut project.figures {
        for node in &mut fig.def.nodes {
            if shift(&mut node.media, removed, old_len) {
                moved += 1;
            }
        }
    }
    for clip in &mut project.clips {
        for node in &mut clip.def.nodes {
            if shift(&mut node.media, removed, old_len) {
                moved += 1;
            }
        }
    }
    for sprite in &mut project.sprites {
        if sprite.asset > removed && sprite.asset < old_len {
            sprite.asset -= 1;
            moved += 1;
        }
    }
    moved
}

fn asset_json(project: &Project, index: usize, asset: &ImageAsset) -> Value {
    let dims = asset_dims(asset);
    let (segments, clip_segments, sprites) = asset_references(project, index);
    json!({
        "index": index,
        "name": asset.name,
        "bytes": asset.png.len(),
        "width": dims.map(|(w, _)| w),
        "height": dims.map(|(_, h)| h),
        "references": {
            "segments": segments,
            "clip_segments": clip_segments,
            "sprites": sprites,
        },
    })
}

/// MCP handler: list the project's asset table — index, name,
/// stored byte size, decoded pixel dimensions (null when the
/// stored bytes do not decode) and live reference counts per
/// asset. Read-only; no undo step.
pub fn tool_assets_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let assets: Vec<Value> = core
        .project
        .assets
        .iter()
        .enumerate()
        .map(|(i, a)| asset_json(&core.project, i, a))
        .collect();
    Ok(json!({
        "ok": true,
        "count": assets.len(),
        "assets": assets,
    }))
}

/// MCP handler: one asset's list entry, addressed by `index`.
/// Read-only; no undo step.
pub fn tool_asset_info(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let mut out = asset_json(&core.project, index, &core.project.assets[index]);
    out["ok"] = json!(true);
    Ok(out)
}

/// MCP handler: import a PNG file from disk into the asset
/// table. The file must start with the PNG magic bytes, decode
/// as an image, and be at most [`ASSET_IMPORT_MAX_BYTES`] —
/// anything else is refused before any snapshot is taken. The
/// asset name defaults to the file's name (the
/// `attach_image_to_node` op's rule); an optional `name`
/// overrides it. The new asset is appended; its index is
/// returned. One undo step.
pub fn tool_asset_import(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = ast_req_str(args, "path")?;
    let meta =
        std::fs::metadata(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if !meta.is_file() {
        return Err(format!("'{path}' is not a file"));
    }
    if meta.len() > ASSET_IMPORT_MAX_BYTES {
        return Err(format!(
            "'{path}' is {} bytes, over the {ASSET_IMPORT_MAX_BYTES}-byte import limit",
            meta.len()
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if !bytes.starts_with(&PNG_MAGIC) {
        return Err(format!("'{path}' is not a PNG (bad magic bytes)"));
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("'{path}' does not decode as an image: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(format!("'{path}' decodes to an empty image"));
    }
    let name = match ast_opt_str(args, "name")? {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image.png".into()),
    };
    let size = bytes.len();
    core.snapshot();
    let index = core.project.assets.len();
    core.project.assets.push(ImageAsset { name: name.clone(), png: bytes });
    core.status = format!("Asset imported ('{name}', asset {}).", index + 1);
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "bytes": size,
        "width": w,
        "height": h,
        "status": core.status,
    }))
}

/// MCP handler: rename asset `index` to `name` (trimmed;
/// empty is refused). Renaming to the current name is a no-op
/// with "changed": false and spends no undo step; a real
/// rename is one undo step.
pub fn tool_asset_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let name = ast_req_str(args, "name")?.trim().to_string();
    if name.is_empty() {
        return Err("argument 'name' must not be empty".into());
    }
    if core.project.assets[index].name == name {
        return Ok(json!({
            "ok": true,
            "index": index,
            "name": name,
            "changed": false,
            "status": core.status,
        }));
    }
    core.snapshot();
    core.project.assets[index].name = name.clone();
    core.status = format!("Asset {} renamed to '{name}'.", index + 1);
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "changed": true,
        "status": core.status,
    }))
}

/// MCP handler: remove asset `index` from the table.
///
/// Removal rule: if ANY reference to the asset exists — a
/// figure segment image, a movieclip-def segment image, or a
/// sprite instance — the removal is REFUSED with the counts,
/// so a reference can never be silently orphaned. When the
/// asset is unreferenced, removal shifts every later asset
/// down one index, and every still-valid reference to a higher
/// index is remapped down by one in the SAME undo step, so
/// references keep naming the same image. (References that
/// were already dangling before the removal are left as-is.)
/// One undo step.
pub fn tool_asset_remove(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let (segments, clip_segments, sprites) = asset_references(&core.project, index);
    if segments + clip_segments + sprites > 0 {
        return Err(format!(
            "asset {index} ('{}') is still referenced by {segments} segment image(s), \
             {clip_segments} clip segment(s) and {sprites} sprite instance(s); \
             remove those references first — removal never orphans a reference",
            core.project.assets[index].name
        ));
    }
    let old_len = core.project.assets.len();
    let name = core.project.assets[index].name.clone();
    core.snapshot();
    core.project.assets.remove(index);
    let remapped = remap_asset_refs(&mut core.project, index, old_len);
    core.status = format!(
        "Asset {} ('{name}') removed; {remapped} reference(s) shifted.",
        index + 1
    );
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "remapped": remapped,
        "count": core.project.assets.len(),
        "status": core.status,
    }))
}

/// MCP handler: attach an EXISTING asset to a segment —
/// `SegmentMedia::Image` on figure `figure`, node `node`,
/// replacing whatever media the segment carried (the
/// `attach_image_to_node` op's overwrite rule, but addressed
/// by index instead of the ./import folder: that op stays as
/// it is). One undo step; the status line is the op's own
/// success string.
pub fn tool_asset_attach(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = ast_req_usize(args, "figure")?;
    let ni = ast_req_usize(args, "node")?;
    let asset = ast_req_usize(args, "asset")?;
    ast_check_node(core, fi, ni)?;
    ast_check_asset(core, asset)?;
    core.snapshot();
    core.project.figures[fi].def.nodes[ni].media = Some(SegmentMedia::Image { asset });
    core.status = format!("Sprite attached (asset {}).", asset + 1);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "asset": asset,
        "status": core.status,
    }))
}

fn composite_image(
    project: &Project,
    asset: usize,
    corners: [V2; 4],
    alpha: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let Some(asset) = project.assets.get(asset) else {
        return;
    };
    let Ok(decoded) = image::load_from_memory(&asset.png) else {
        return;
    };
    let src = decoded.to_rgba8();
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    if sw < 1.0 || sh < 1.0 {
        return;
    }
    let c: [[f32; 2]; 4] = [
        view(corners[0]),
        view(corners[1]),
        view(corners[2]),
        view(corners[3]),
    ];
    let e1 = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
    let e2 = [c[3][0] - c[0][0], c[3][1] - c[0][1]];
    let det = e1[0] * e2[1] - e1[1] * e2[0];
    if det.abs() <= f32::EPSILON {
        return;
    }
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    let min_x = c.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_x = c.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    let min_y = c.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_y = c.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    for y in min_y.max(0)..=max_y.min(ih - 1) {
        for x in min_x.max(0)..=max_x.min(iw - 1) {
            let rel = [x as f32 + 0.5 - c[0][0], y as f32 + 0.5 - c[0][1]];
            let u = (rel[0] * e2[1] - rel[1] * e2[0]) / det;
            let v = (e1[0] * rel[1] - e1[1] * rel[0]) / det;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            let sx = ((u * sw) as u32).min(src.width() - 1);
            let sy = ((v * sh) as u32).min(src.height() - 1);
            let sp = *src.get_pixel(sx, sy);
            let sa = sp[3] as f32 / 255.0 * alpha;
            if sa <= 0.0 {
                continue;
            }
            let dp = img.get_pixel_mut(x as u32, y as u32);
            let da = dp[3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }
            for ch in 0..3 {
                let blended =
                    (sp[ch] as f32 * sa + dp[ch] as f32 * da * (1.0 - sa)) / out_a;
                dp[ch] = blended.round().clamp(0.0, 255.0) as u8;
            }
            dp[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_text(
    pos: V2,
    angle_deg: f32,
    content: &str,
    size: f32,
    color: u32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
    view_scale: f32,
) {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() || size <= 0.0 {
        return;
    }
    let cell = size / 5.0; // stage units per glyph pixel
    let total_units = chars.len() as f32 * 4.0 - 1.0; // 3 wide + 1 gap
    let start = -total_units / 2.0;
    let rad = angle_deg.to_radians();
    let (cs, sn) = (rad.cos(), rad.sin());
    let rgba = Rgba([
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        255,
    ]);
    let block = (cell * view_scale).round().max(1.0) as i32;
    for (ci, ch) in chars.iter().enumerate() {
        let glyph = glyph_rows(*ch);
        for row in 0..5usize {
            for col in 0..3usize {
                if (glyph[row] >> (2 - col)) & 1 == 0 {
                    continue;
                }
                let lx = (start + ci as f32 * 4.0 + col as f32 + 0.5) * cell;
                let ly = (row as f32 - 2.0 + 0.5) * cell;
                let world = [
                    pos[0] + lx * cs - ly * sn,
                    pos[1] + lx * sn + ly * cs,
                ];
                let px = view(world);
                let cx = px[0].round() as i32;
                let cy = px[1].round() as i32;
                for dy in 0..block {
                    for dx in 0..block {
                        let x = cx - block / 2 + dx;
                        let y = cy - block / 2 + dy;
                        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height()
                        {
                            img.put_pixel(x as u32, y as u32, rgba);
                        }
                    }
                }
            }
        }
    }
}

/// 3x5 bitmap glyph for `ch`: five rows, 3 bits each, bit 2 is the
/// leftmost pixel. Lowercase folds to uppercase; unknown printable
/// characters fall back to `?`.
fn glyph_rows(ch: char) -> [u8; 5] {
    let up = ch.to_ascii_uppercase();
    match up {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b010, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        ' ' => [0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0b010],
        ',' => [0, 0, 0, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '?' => [0b110, 0b001, 0b010, 0, 0b010],
        '\'' => [0b010, 0b010, 0, 0, 0],
        '"' => [0b101, 0b101, 0, 0, 0],
        '-' => [0, 0, 0b111, 0, 0],
        ':' => [0, 0b010, 0, 0b010, 0],
        ';' => [0, 0b010, 0, 0b010, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '+' => [0, 0b010, 0b111, 0b010, 0],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '*' => [0, 0b101, 0b010, 0b101, 0],
        '=' => [0, 0b111, 0, 0b111, 0],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '_' => [0, 0, 0, 0, 0b111],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        _ => {
            if ch.is_control() {
                [0, 0, 0, 0, 0]
            } else {
                glyph_rows('?')
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Easing, FigureDef, FigureInstance, Frame, ImageAsset, NodeDef, NodeStyle, Pose, Shape,
        SpriteInstance, TextInstance,
    };
    use std::io::Cursor;

    fn png_bytes(w: u32, h: u32, px: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, px);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn node(
        name: &str,
        parent: Option<usize>,
        length: f32,
        media: Option<SegmentMedia>,
    ) -> NodeDef {
        NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media,
        }
    }

    /// One figure, root at (100, 100): an image segment running
    /// +x for 100 units (pose angle 90 in this model's dir()), and
    /// a text segment running down (+y) for 80. Asset 0 is a 4x2
    /// green PNG (aspect 2.0).
    fn demo_project() -> Project {
        let fig = FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "Media".into(),
            color: 0xFF0000,
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node(
                        "sprite",
                        Some(0),
                        100.0,
                        Some(SegmentMedia::Image { asset: 0 }),
                    ),
                    node(
                        "label",
                        Some(0),
                        80.0,
                        Some(SegmentMedia::Text {
                            content: "HI".into(),
                            size: 25.0,
                        }),
                    ),
                ],
            },
            hidden: false,
            clip: None,
        };
        Project {
            name: "media-test".into(),
            fps: 12.0,
            stage: [800.0, 600.0],
            background: 0,
            figures: vec![fig],
            frames: vec![Frame {
                poses: vec![Pose {
                    root: [100.0, 100.0],
                    angles: vec![0.0, 90.0, 0.0],
                }],
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            }],
            camera: Vec::new(),
            tween_playback: false,
            easing: Easing::Linear,
            camera_follow: None,
            particles: Vec::new(),
            strokes: Vec::new(),
            audio: Vec::new(),
            clips: Vec::new(),
            fx_clips: Vec::new(),
            assets: vec![ImageAsset {
                name: "green".into(),
                png: png_bytes(4, 2, Rgba([0, 200, 0, 255])),
            }],
            channels: Vec::new(),
            sprites: Vec::new(),
            texts: Vec::new(),
            weather: None,
            head_ring_scale: 1.0,
        }
    }

    fn close(a: V2, b: V2) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    #[test]
    fn image_item_quad_follows_segment_with_png_aspect() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let img = items.iter().find_map(|i| match i {
            MediaItem::Image { corners, asset, .. } => Some((corners, asset)),
            _ => None,
        });
        let (corners, asset) = img.expect("an image item");
        assert_eq!(*asset, 0);
        // Wave 12 (A6 §3a): a default figure/style carries
        // alpha 1.0 — the opacity product starts at unity.
        // Segment (100,100)->(200,100); aspect 2 -> height 50.
        assert!(close(corners[0], [100.0, 75.0]), "c0 {:?}", corners[0]);
        assert!(close(corners[1], [200.0, 75.0]), "c1 {:?}", corners[1]);
        assert!(close(corners[2], [200.0, 125.0]), "c2 {:?}", corners[2]);
        assert!(close(corners[3], [100.0, 125.0]), "c3 {:?}", corners[3]);
        // Long-edge midpoints lie exactly on the segment.
        let top_mid = scale(add(corners[0], corners[1]), 0.5);
        assert!(close(top_mid, [150.0, 75.0]));
    }

    #[test]
    fn image_item_alpha_is_figure_opacity_times_style_opacity() {
        // Wave 12 (A6 §3a): segment media used to composite at
        // full strength whatever the figure or the node style
        // said. The item now carries the product of both.
        let alpha_of = |project: &Project| -> f32 {
            media_items(project, 0.0)
                .iter()
                .find_map(|i| match i {
                    MediaItem::Image { alpha, .. } => Some(*alpha),
                    _ => None,
                })
                .expect("an image item")
        };
        let project = demo_project();
        assert_eq!(alpha_of(&project), 1.0, "defaults multiply to unity");
        let mut project = demo_project();
        project.figures[0].opacity = 0.5;
        assert_eq!(alpha_of(&project), 0.5, "figure opacity alone");
        project.figures[0].def.nodes[1].style.opacity = 0.5;
        assert_eq!(alpha_of(&project), 0.25, "figure × style");
        // Out-of-range stored values clamp per factor, exactly
        // as the segment's own alpha does.
        project.figures[0].opacity = 1.4;
        project.figures[0].def.nodes[1].style.opacity = 2.0;
        assert_eq!(alpha_of(&project), 1.0, "each factor clamps to 1");
    }

    #[test]
    fn text_item_sits_at_midpoint_along_segment() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let text = items.iter().find_map(|i| match i {
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
            } => Some((pos, angle_deg, content, size, color)),
            _ => None,
        });
        let (pos, angle, content, size, color) = text.expect("a text item");
        assert!(close(*pos, [100.0, 140.0]), "pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "angle {angle}");
        assert_eq!(content, "HI");
        assert_eq!(*size, 25.0);
        assert_eq!(*color, 0xFF0000, "falls back to the figure colour");
    }

    #[test]
    fn composite_paints_image_quad_and_text_pixels() {
        let project = demo_project();
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // Centre of the sprite quad is the asset's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([0, 200, 0, 255]));
        // Inside the quad but off-centre, too.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // A known "on" cell of the H: text runs downward from
        // (100, 140); the first glyph cell centre lands at
        // (107.5, 125), just below the sprite quad's bottom edge.
        assert_eq!(*img.get_pixel(108, 125), Rgba([255, 0, 0, 255]));
        // Far corner untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn missing_or_broken_assets_are_skipped() {
        let mut project = demo_project();
        project.assets.clear();
        project.assets.push(ImageAsset {
            name: "junk".into(),
            png: vec![1, 2, 3],
        });
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "only the text item survives");
        assert!(matches!(items[0], MediaItem::Text { .. }));
    }

    #[test]
    fn glyph_table_covers_charset_and_folds_case() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!?'-:;()+/&%*=<>_#$@\"".chars() {
            let g = glyph_rows(ch);
            let lit: u32 = g.iter().map(|r| r.count_ones()).sum();
            assert!(lit > 0, "glyph for {ch:?} is blank");
        }
        assert_eq!(glyph_rows('a'), glyph_rows('A'));
        assert_eq!(glyph_rows('~'), glyph_rows('?'), "unknown -> ?");
    }

    fn image_items(items: &[MediaItem]) -> Vec<([V2; 4], usize)> {
        items
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, asset, .. } => Some((*corners, *asset)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn sprite_instance_item_geometry_and_sampling() {
        let mut project = demo_project();
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![
                InstanceKey {
                    frame: 0.0,
                    x: 300.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 0.0,
                },
                InstanceKey {
                    frame: 2.0,
                    x: 340.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 90.0,
                },
            ],
        });
        // Frame 0: asset is 4x2 px, scale 10 -> 40x20 quad at
        // (300, 200), unrotated.
        let items = media_items(&project, 0.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [280.0, 190.0]), "c0 {corners:?}");
        assert!(close(corners[1], [320.0, 190.0]), "c1 {corners:?}");
        assert!(close(corners[2], [320.0, 210.0]), "c2 {corners:?}");
        assert!(close(corners[3], [280.0, 210.0]), "c3 {corners:?}");
        // Frame 1: keys interpolate — centre (320, 200).
        let items = media_items(&project, 1.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        let centre = scale(add(corners[0], corners[2]), 0.5);
        assert!(close(centre, [320.0, 200.0]), "centre {centre:?}");
        // Frame 2: rotated 90 degrees — the quad is 20 wide, 40 tall.
        let items = media_items(&project, 2.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [350.0, 180.0]), "c0 {corners:?}");
        assert!(close(corners[1], [350.0, 220.0]), "c1 {corners:?}");
    }

    #[test]
    fn text_instance_item_uses_size_times_key_scale() {
        let mut project = demo_project();
        project.texts.push(TextInstance {
            content: "YO".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 400.0,
                scale: 2.0,
                angle: 0.0,
            }],
        });
        let items = media_items(&project, 0.0);
        // The instance text comes after the segment "HI" text.
        let text = items.iter().rev().find_map(|i| match i {
            MediaItem::Text {
                pos,
                content,
                size,
                color,
                ..
            } => Some((pos, content, size, color)),
            _ => None,
        });
        let (pos, content, size, color) = text.expect("a text instance item");
        assert!(close(*pos, [100.0, 400.0]), "pos {pos:?}");
        assert_eq!(content, "YO");
        assert_eq!(*size, 50.0, "25 * key scale 2");
        assert_eq!(*color, 0x0000FF);
    }

    #[test]
    fn composite_instances_draw_over_segment_media() {
        let mut project = demo_project();
        project.assets.push(ImageAsset {
            name: "red".into(),
            png: png_bytes(2, 2, Rgba([255, 0, 0, 255])),
        });
        // Red sprite instance (2x2 px * scale 10 = 20x20) centred
        // on the segment image's centre (150, 100).
        project.sprites.push(SpriteInstance {
            asset: 1,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 150.0,
                y: 100.0,
                scale: 10.0,
                angle: 0.0,
            }],
        });
        project.texts.push(TextInstance {
            content: "H".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 250.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // The instance sprite paints over the segment's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([255, 0, 0, 255]));
        // Green still visible outside the instance quad.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // The text instance stamps blue glyph pixels in its band.
        let mut blue = 0;
        for y in 238..262 {
            for x in 80..120 {
                if *img.get_pixel(x, y) == Rgba([0, 0, 255, 255]) {
                    blue += 1;
                }
            }
        }
        assert!(blue > 10, "text instance painted only {blue} blue pixels");
    }

    // ---- Wave 12: asset-table MCP handlers ----

    use crate::editor::EditorCore;
    use crate::model::{ClipDef, ClipLoop};
    use serde_json::json;

    fn asset_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project = demo_project();
        core
    }

    fn temp_asset_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stykanimate-assets-{tag}-{}.png",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn asset_import_refuses_bad_magic_oversize_and_undecodable() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // Bad magic: not a PNG at all.
        let bad = temp_asset_file("badmagic", b"this is not a png file");
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": bad.to_string_lossy() }),
        )
        .expect_err("bad magic must be refused");
        assert!(err.contains("not a PNG"), "err: {err}");
        // Oversize: valid magic, but over the byte cap (the size
        // check fires on metadata before any decode).
        let mut big = PNG_MAGIC.to_vec();
        big.resize(ASSET_IMPORT_MAX_BYTES as usize + 1, 0);
        let big_path = temp_asset_file("oversize", &big);
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": big_path.to_string_lossy() }),
        )
        .expect_err("oversize must be refused");
        assert!(err.contains("import limit"), "err: {err}");
        // Valid magic, undecodable body.
        let mut junk = PNG_MAGIC.to_vec();
        junk.extend_from_slice(b"garbagegarbage");
        let junk_path = temp_asset_file("undecodable", &junk);
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": junk_path.to_string_lossy() }),
        )
        .expect_err("undecodable must be refused");
        assert!(err.contains("does not decode"), "err: {err}");
        // Nothing was stored and no undo step was spent.
        assert_eq!(core.project.assets.len(), 1);
        assert_eq!(core.undo_depth(), undo_before);
        for p in [bad, big_path, junk_path] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn asset_import_appends_one_asset_in_one_undo_step() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        let bytes = png_bytes(6, 3, Rgba([9, 9, 9, 255]));
        let path = temp_asset_file("ok", &bytes);
        let out = tool_asset_import(
            &mut core,
            &json!({ "path": path.to_string_lossy() }),
        )
        .expect("a valid PNG imports");
        assert_eq!(out["index"].as_u64(), Some(1));
        assert_eq!(out["width"].as_u64(), Some(6));
        assert_eq!(out["height"].as_u64(), Some(3));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].png, bytes);
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one undo step");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn asset_remove_refused_while_any_reference_kind_points_at_it() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // (a) The demo's segment image references asset 0.
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("segment-referenced asset must be refused");
        assert!(err.contains("1 segment image(s)"), "err: {err}");
        // (b) Clear the segment; a sprite instance references it.
        core.project.figures[0].def.nodes[1].media = None;
        core.project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("sprite-referenced asset must be refused");
        assert!(err.contains("1 sprite instance(s)"), "err: {err}");
        // (c) Clear the sprite; a movieclip def references it.
        core.project.sprites.clear();
        core.project.clips.push(ClipDef {
            name: "clip".into(),
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node("img", Some(0), 50.0, Some(SegmentMedia::Image { asset: 0 })),
                ],
            },
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("clip-referenced asset must be refused");
        assert!(err.contains("1 clip segment(s)"), "err: {err}");
        // Refusals never mutate and never snapshot.
        assert_eq!(core.project.assets.len(), 1);
        assert_eq!(core.undo_depth(), undo_before);
    }

    #[test]
    fn asset_remove_unreferenced_remaps_higher_references_in_same_step() {
        let mut core = asset_core();
        // Three assets; the demo segment and a sprite both use
        // asset 2; asset 0 is unreferenced.
        core.project.assets.push(ImageAsset {
            name: "spare".into(),
            png: png_bytes(2, 2, Rgba([1, 2, 3, 255])),
        });
        core.project.assets.push(ImageAsset {
            name: "hero".into(),
            png: png_bytes(2, 2, Rgba([4, 5, 6, 255])),
        });
        core.project.figures[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 2 });
        core.project.sprites.push(SpriteInstance {
            asset: 2,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        let undo_before = core.undo_depth();
        let out = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect("unreferenced removal succeeds");
        assert_eq!(out["remapped"].as_u64(), Some(2));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].name, "hero");
        // Both references followed their image down one index.
        assert_eq!(
            core.project.figures[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 1 })
        );
        assert_eq!(core.project.sprites[0].asset, 1);
        assert_eq!(core.undo_depth(), undo_before + 1, "remove + remap = one undo step");
    }

    #[test]
    fn asset_attach_uses_an_existing_asset_and_validates_indices() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // Node 2 carries the demo's text; attach replaces it.
        let out = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 0 }),
        )
        .expect("attach succeeds");
        assert_eq!(out["status"].as_str(), Some("Sprite attached (asset 1)."));
        assert_eq!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { asset: 0 })
        );
        assert_eq!(core.undo_depth(), undo_before + 1);
        // A missing asset index is an error, not a snapshot.
        let err = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 7 }),
        )
        .expect_err("missing asset must be refused");
        assert!(err.contains("out of range"), "err: {err}");
        assert_eq!(core.undo_depth(), undo_before + 1);
    }

    #[test]
    fn assets_list_and_info_report_dimensions_and_references() {
        let mut core = asset_core();
        let out = tool_assets_list(&mut core, &json!({})).expect("list succeeds");
        assert_eq!(out["count"].as_u64(), Some(1));
        let a = &out["assets"][0];
        assert_eq!(a["name"].as_str(), Some("green"));
        assert_eq!(a["width"].as_u64(), Some(4));
        assert_eq!(a["height"].as_u64(), Some(2));
        assert!(a["bytes"].as_u64().unwrap_or(0) > 0);
        assert_eq!(a["references"]["segments"].as_u64(), Some(1));
        assert_eq!(a["references"]["sprites"].as_u64(), Some(0));
        let info = tool_asset_info(&mut core, &json!({ "index": 0 })).expect("info succeeds");
        assert_eq!(info["ok"].as_bool(), Some(true));
        assert_eq!(info["name"].as_str(), Some("green"));
        let err = tool_asset_info(&mut core, &json!({ "index": 3 }))
            .expect_err("missing asset must be refused");
        assert!(err.contains("out of range"), "err: {err}");
    }

    #[test]
    fn asset_rename_spends_an_undo_step_only_on_a_real_change() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        let out = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "hero" }),
        )
        .expect("rename succeeds");
        assert_eq!(out["changed"].as_bool(), Some(true));
        assert_eq!(core.project.assets[0].name, "hero");
        assert_eq!(core.undo_depth(), undo_before + 1);
        let out = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "hero" }),
        )
        .expect("same-name rename is a no-op, not an error");
        assert_eq!(out["changed"].as_bool(), Some(false));
        assert_eq!(core.undo_depth(), undo_before + 1, "no-op spends no snapshot");
        let err = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "   " }),
        )
        .expect_err("blank name must be refused");
        assert!(err.contains("must not be empty"), "err: {err}");
    }
}
