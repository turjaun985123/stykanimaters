//! Layered-set import (Wave Y, P-9): bring a SpriteLoop
//! import package — or a plain folder of images — into the
//! project as dressing for the stick figure.
//!
//! The package is the format the official Krita and
//! Photoshop exporters emit (documented in
//! `~/workspace/spriteloop-re/findings/src-4.md`): a folder
//! holding `spriteloop.import.json` plus one cropped PNG
//! per part under `images/`. The manifest carries each
//! part's canvas-space bounds, opacity and visibility, and
//! an implicit z-order (array order). It carries NO rig
//! semantics — no pivots, no bones, no naming language —
//! and this importer creates none: parts attach to the
//! target figure's EXISTING nodes by name, and a part with
//! no matching node degrades to a free sprite. Layers dress
//! stick nodes; they never build the character.
//!
//! Discipline, inherited from the asset funnel
//! (`crate::media::tool_asset_import`): EVERYTHING is
//! validated before any write — manifest shape, part
//! bounds, image paths (which must stay inside the package
//! folder), image existence, and the shared content gate
//! (`crate::media::asset_from_bytes`). One corrupt part
//! refuses the whole import with no snapshot spent. A
//! successful import spends exactly ONE undo step for all
//! of its assets, attachments and sprites.
//!
//! Integrator wiring: declare `pub mod layered_import;` in
//! `lib.rs` (alphabetical: before `pub mod layers;`).
//! Nothing else is needed — this module only adds free
//! functions over `EditorCore`. The GUI command and the
//! MCP registry tool are Y6's surface, built on
//! [`import_layered_package`] / [`import_layered_folder`].

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::editor::EditorCore;
use crate::media::asset_from_bytes;
use crate::model::{ImageAsset, InstanceKey, SegmentMedia, SpriteInstance};

/// What an import did, for the calling surface (Y6's GUI
/// command / MCP tool) to report. Names are the parts'
/// manifest names (or file stems, for a folder import), in
/// package order within each list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayeredImportReport {
    /// Total parts in the set.
    pub parts: usize,
    /// Assets appended to the project table (one per part).
    pub assets_added: usize,
    /// Names of parts attached to a figure node as segment
    /// media.
    pub attached: Vec<String>,
    /// Names of parts that became free sprites (no name
    /// match, an invisible part, or a node already dressed
    /// by an earlier part of the same import).
    pub sprites: Vec<String>,
    /// Attached parts whose manifest opacity was < 1.0
    /// and is now STORED on the attached segment media
    /// (Wave Z, Z5 — the payload's own `opacity` field;
    /// this field was `attached_opacity_dropped` while
    /// segment media had no opacity home, and the count
    /// survives under its honest name: nothing is
    /// dropped any more).
    pub attached_opacity_stored: usize,
}

// ---------------------------------------------------------------------
// Manifest shape (the exporters' shared schema, v1)
// ---------------------------------------------------------------------

#[derive(Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    canvas: Canvas,
    #[serde(default)]
    parts: Vec<ManifestPart>,
}

#[derive(Deserialize)]
struct Canvas {
    width: i64,
    height: i64,
}

#[derive(Deserialize)]
struct ManifestPart {
    id: String,
    name: String,
    image: String,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    #[serde(default = "default_part_opacity")]
    opacity: f32,
    #[serde(default = "default_part_visible")]
    visible: bool,
    // `parentId` and the top-level `hierarchy` are
    // deliberately not modelled: the format spec's own
    // importer note blesses ignoring them for placement,
    // and group structure must never become rig data.
}

fn default_part_opacity() -> f32 {
    1.0
}

fn default_part_visible() -> bool {
    true
}

/// The exporters' identity slug (identical logic in the
/// Krita and Photoshop plugins): trim, lowercase, every run
/// of non-`[a-z0-9]` becomes one `_`, leading/trailing `_`
/// stripped, empty becomes `"part"`. Part ids ARE slugs;
/// node names meet them here (`upper-arm L` and
/// `Upper Arm L` both slug to `upper_arm_l`).
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_us = true; // also strips a leading run
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            pending_us = false;
        } else if !pending_us {
            out.push('_');
            pending_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "part".to_string()
    } else {
        out
    }
}

/// A part that has passed every validation, image bytes
/// already funnel-checked, ready for the apply pass.
struct ReadyPart {
    name: String,
    /// Slug keys this part answers to (its manifest id and
    /// its slugified display name; one key for folder parts).
    match_keys: Vec<String>,
    asset: ImageAsset,
    /// Canvas-space bounds (package imports only; a folder
    /// part has no canvas geometry).
    bounds: Option<(f32, f32, f32, f32)>,
    opacity: f32,
    visible: bool,
}

fn check_figure(core: &EditorCore, figure: usize) -> Result<(), String> {
    let n = core.project.figures.len();
    if figure >= n {
        return Err(format!(
            "figure index {figure} out of range (project has {n} figure(s))"
        ));
    }
    Ok(())
}

/// Resolve a manifest `image` path inside the package
/// folder. Absolute paths and `..` components refuse: a
/// package must never reach outside its own folder.
fn resolve_image(dir: &Path, image: &str, part: &str) -> Result<PathBuf, String> {
    let rel = Path::new(image);
    if rel.is_absolute() {
        return Err(format!(
            "layered import refused: part '{part}' has an absolute image path '{image}'"
        ));
    }
    for comp in rel.components() {
        if matches!(comp, Component::ParentDir | Component::RootDir)
            || matches!(comp, Component::Prefix(_))
        {
            return Err(format!(
                "layered import refused: part '{part}' image path '{image}' escapes the package folder"
            ));
        }
    }
    Ok(dir.join(rel))
}

/// Import the SpriteLoop package rooted at `dir` (the
/// folder containing `spriteloop.import.json`), dressing
/// figure `figure`. See the module docs for the validation
/// and placement rules. One undo step on success; a
/// refusal changes nothing and spends nothing.
pub fn import_layered_package(
    core: &mut EditorCore,
    dir: &Path,
    figure: usize,
) -> Result<LayeredImportReport, String> {
    check_figure(core, figure)?;
    let manifest_path = dir.join("spriteloop.import.json");
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "layered import refused: cannot read '{}': {e}",
            manifest_path.display()
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| {
        format!(
            "layered import refused: '{}' is not a valid spriteloop.import package: {e}",
            manifest_path.display()
        )
    })?;
    if manifest.format != "spriteloop.import" {
        return Err(format!(
            "layered import refused: format is '{}', not 'spriteloop.import'",
            manifest.format
        ));
    }
    if manifest.version != 1 {
        return Err(format!(
            "layered import refused: version {} is not supported (only version 1)",
            manifest.version
        ));
    }
    if manifest.canvas.width < 1 || manifest.canvas.height < 1 {
        return Err(
            "layered import refused: canvas width/height must be at least 1".into(),
        );
    }
    if manifest.parts.is_empty() {
        return Err("layered import refused: the package contains no parts".into());
    }
    // Validate every part — fields, path, file, content —
    // before the apply pass writes anything.
    let mut ready = Vec::with_capacity(manifest.parts.len());
    for part in &manifest.parts {
        if part.width < 1 || part.height < 1 {
            return Err(format!(
                "layered import refused: part '{}' has non-positive bounds {}x{}",
                part.name, part.width, part.height
            ));
        }
        if !part.opacity.is_finite() {
            return Err(format!(
                "layered import refused: part '{}' has a non-finite opacity",
                part.name
            ));
        }
        let image_path = resolve_image(dir, &part.image, &part.name)?;
        let bytes = std::fs::read(&image_path).map_err(|e| {
            format!(
                "layered import refused: part '{}' image '{}' cannot be read: {e}",
                part.name, part.image
            )
        })?;
        let (asset, _, _) = asset_from_bytes(part.name.clone(), bytes)
            .map_err(|e| format!("layered import refused: part '{}': {e}", part.name))?;
        let mut match_keys = vec![slugify(&part.id), slugify(&part.name)];
        match_keys.dedup();
        ready.push(ReadyPart {
            name: part.name.clone(),
            match_keys,
            asset,
            bounds: Some((
                part.x as f32,
                part.y as f32,
                part.width as f32,
                part.height as f32,
            )),
            opacity: part.opacity.clamp(0.0, 1.0),
            visible: part.visible,
        });
    }
    let canvas = (manifest.canvas.width as f32, manifest.canvas.height as f32);
    Ok(apply_parts(core, figure, ready, Some(canvas)))
}

/// Import a plain folder of images: every `.png` / `.jpg`
/// / `.jpeg` file in `dir` (sorted by file name) becomes a
/// part named by its file stem. If the folder holds a
/// `spriteloop.import.json`, this delegates to
/// [`import_layered_package`] — the manifest always wins.
pub fn import_layered_folder(
    core: &mut EditorCore,
    dir: &Path,
    figure: usize,
) -> Result<LayeredImportReport, String> {
    check_figure(core, figure)?;
    if dir.join("spriteloop.import.json").is_file() {
        return import_layered_package(core, dir, figure);
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        format!("layered import refused: cannot list '{}': {e}", dir.display())
    })?;
    for entry in entries {
        let path = entry
            .map_err(|e| {
                format!("layered import refused: cannot list '{}': {e}", dir.display())
            })?
            .path();
        if !path.is_file() {
            continue;
        }
        let is_image = path
            .extension()
            .map(|e| {
                let e = e.to_string_lossy().to_lowercase();
                e == "png" || e == "jpg" || e == "jpeg"
            })
            .unwrap_or(false);
        if is_image {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "layered import refused: '{}' contains no PNG or JPEG images",
            dir.display()
        ));
    }
    let mut ready = Vec::with_capacity(files.len());
    for path in &files {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        let bytes = std::fs::read(path).map_err(|e| {
            format!("layered import refused: cannot read '{}': {e}", path.display())
        })?;
        let (asset, _, _) = asset_from_bytes(stem.clone(), bytes)
            .map_err(|e| format!("layered import refused: '{stem}': {e}"))?;
        ready.push(ReadyPart {
            match_keys: vec![slugify(&stem)],
            name: stem,
            asset,
            bounds: None,
            opacity: 1.0,
            visible: true,
        });
    }
    Ok(apply_parts(core, figure, ready, None))
}

/// The one mutating pass: a single snapshot, then assets,
/// attachments and sprites in package order.
fn apply_parts(
    core: &mut EditorCore,
    figure: usize,
    ready: Vec<ReadyPart>,
    canvas: Option<(f32, f32)>,
) -> LayeredImportReport {
    let stage = core.project.stage;
    let node_keys: Vec<String> = core.project.figures[figure]
        .def
        .nodes
        .iter()
        .map(|n| slugify(&n.name))
        .collect();
    let mut report = LayeredImportReport {
        parts: ready.len(),
        ..Default::default()
    };
    core.snapshot();
    // Nodes dressed by THIS import: a later part matching
    // only an already-dressed node degrades to a sprite
    // instead of silently overwriting the earlier part.
    let mut dressed: Vec<bool> = vec![false; node_keys.len()];
    for part in ready {
        let asset_index = core.project.assets.len();
        let target = if part.visible {
            node_keys.iter().enumerate().find_map(|(ni, key)| {
                if !dressed[ni] && part.match_keys.iter().any(|k| k == key) {
                    Some(ni)
                } else {
                    None
                }
            })
        } else {
            None
        };
        // The sprite key borrows the part's geometry, so it
        // is computed before the asset moves into the table.
        let key = if target.is_none() {
            Some(sprite_key(&part, canvas, stage))
        } else {
            None
        };
        core.project.assets.push(part.asset);
        report.assets_added += 1;
        match target {
            Some(ni) => {
                dressed[ni] = true;
                if part.opacity < 1.0 {
                    report.attached_opacity_stored += 1;
                }
                // Wave Z (Z5): the part's opacity is stored
                // on the segment media itself (the payload's
                // `opacity` field, which the media funnel
                // multiplies into the item alpha) — at 1.0
                // the stored value is the serde default and
                // serialises away, exactly like an attach
                // from any other surface.
                core.project.figures[figure].def.nodes[ni].media =
                    Some(SegmentMedia::Image { asset: asset_index, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: part.opacity });
                report.attached.push(part.name);
            }
            None => {
                core.project.sprites.push(SpriteInstance {
                    asset: asset_index,
                    keys: vec![key.expect("sprite key computed for a degrading part")],
                });
                report.sprites.push(part.name);
            }
        }
    }
    core.status = format!(
        "Layered set imported: {} part(s), {} attached to figure {}, {} as sprite(s).",
        report.parts,
        report.attached.len(),
        figure + 1,
        report.sprites.len(),
    );
    report
}

/// The frame-0 key for a part that became a free sprite.
/// With canvas geometry the key places the sprite exactly
/// over its mapped bounds (per-axis stage mapping, centre
/// pivot); without it, the sprite lands at the stage
/// centre at scale 1 (the `add_sprite` placement).
fn sprite_key(
    part: &ReadyPart,
    canvas: Option<(f32, f32)>,
    stage: [f32; 2],
) -> InstanceKey {
    let (x, y, scale, scale_y) = match (canvas, part.bounds) {
        (Some((cw, ch)), Some((bx, by, bw, bh))) => {
            let sx = stage[0] / cw;
            let sy = stage[1] / ch;
            ((bx + bw / 2.0) * sx, (by + bh / 2.0) * sy, sx, sy)
        }
        _ => (stage[0] / 2.0, stage[1] / 2.0, 1.0, 0.0),
    };
    InstanceKey {
        frame: 0.0,
        x,
        y,
        scale,
        angle: 0.0,
        opacity: if part.visible { part.opacity } else { 0.0 },
        flip_x: false,
        flip_y: false,
        scale_y,
        pivot_x: 0.5,
        pivot_y: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([200, 40, 40, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-layered-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        core
    }

    fn write_package(dir: &Path, manifest: &serde_json::Value, images: &[(&str, Vec<u8>)]) {
        std::fs::create_dir_all(dir.join("images")).unwrap();
        std::fs::write(
            dir.join("spriteloop.import.json"),
            serde_json::to_string(manifest).unwrap(),
        )
        .unwrap();
        for (name, bytes) in images {
            std::fs::write(dir.join("images").join(name), bytes).unwrap();
        }
    }

    #[test]
    fn package_import_attaches_by_name_and_degrades_the_rest() {
        let mut core = core();
        let (assets0, sprites0, undo0) = (
            core.project.assets.len(),
            core.project.sprites.len(),
            core.undo_depth(),
        );
        let dir = temp_dir("attach");
        let manifest = serde_json::json!({
            "format": "spriteloop.import",
            "version": 1,
            "canvas": { "width": 960, "height": 720 },
            "parts": [
                { "id": "head", "name": "Head", "image": "images/head.png",
                  "x": 400, "y": 100, "width": 80, "height": 60, "opacity": 0.5 },
                { "id": "upper_arm_l", "name": "Upper Arm L", "image": "images/upper_arm_l.png",
                  "x": 300, "y": 200, "width": 40, "height": 90 },
                { "id": "cape", "name": "Cape", "image": "images/cape.png",
                  "x": 100, "y": 100, "width": 50, "height": 50, "opacity": 0.5 }
            ]
        });
        write_package(
            &dir,
            &manifest,
            &[
                ("head.png", png_bytes(80, 60)),
                ("upper_arm_l.png", png_bytes(40, 90)),
                ("cape.png", png_bytes(50, 50)),
            ],
        );
        let report = import_layered_package(&mut core, &dir, 0).expect("package imports");
        assert_eq!(report.parts, 3);
        assert_eq!(report.attached, vec!["Head", "Upper Arm L"]);
        assert_eq!(report.sprites, vec!["Cape"]);
        assert_eq!(core.project.assets.len(), assets0 + 3);
        // "Head" dressed the humanoid's head node (index 2);
        // "Upper Arm L" met node `upper-arm L` (index 3) in
        // slug space.
        let head_asset = match &core.project.figures[0].def.nodes[2].media {
            Some(SegmentMedia::Image { asset, .. }) => *asset,
            other => panic!("head node media: {other:?}"),
        };
        assert_eq!(core.project.assets[head_asset].name, "Head");
        // Wave Z (Z5): the Head part's manifest opacity
        // (0.5) is STORED on the attached segment media —
        // the payload's own opacity field — and counted
        // as stored, not dropped; the funnel then blends
        // the head image at that alpha (figure and style
        // opacities are 1 in the demo project).
        assert_eq!(report.attached_opacity_stored, 1);
        match &core.project.figures[0].def.nodes[2].media {
            Some(SegmentMedia::Image { opacity, .. }) => {
                assert_eq!(*opacity, 0.5, "manifest opacity stored on the media")
            }
            other => panic!("head node media: {other:?}"),
        }
        let head_alpha = crate::media::media_items(&core.project, 0.0)
            .iter()
            .find_map(|i| match i {
                crate::media::MediaItem::Image { asset, alpha, .. }
                    if *asset == head_asset =>
                {
                    Some(*alpha)
                }
                _ => None,
            })
            .expect("the head media emits an item");
        assert_eq!(head_alpha, 0.5, "the funnel blends at the stored opacity");
        assert!(matches!(
            core.project.figures[0].def.nodes[3].media,
            Some(SegmentMedia::Image { .. })
        ));
        // The cape is a free sprite over its mapped bounds:
        // canvas == stage here, so centre (125, 125), scale
        // 1 on both axes, manifest opacity carried.
        assert_eq!(core.project.sprites.len(), sprites0 + 1);
        let sprite = &core.project.sprites[sprites0];
        assert_eq!(core.project.assets[sprite.asset].name, "Cape");
        let key = sprite.keys[0];
        assert_eq!((key.x, key.y), (125.0, 125.0));
        assert_eq!((key.scale, key.eff_scale_y()), (1.0, 1.0));
        assert_eq!(key.opacity, 0.5);
        assert_eq!(core.undo_depth(), undo0 + 1, "exactly one undo step");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_packages_refuse_before_any_write() {
        let cases: Vec<(&str, serde_json::Value, Vec<(&str, Vec<u8>)>)> = vec![
            (
                "format",
                serde_json::json!({
                    "format": "something.else", "version": 1,
                    "canvas": { "width": 100, "height": 100 },
                    "parts": [{ "id": "head", "name": "Head", "image": "images/head.png",
                                "x": 0, "y": 0, "width": 10, "height": 10 }]
                }),
                vec![("head.png", png_bytes(10, 10))],
            ),
            (
                "missing",
                serde_json::json!({
                    "format": "spriteloop.import", "version": 1,
                    "canvas": { "width": 100, "height": 100 },
                    "parts": [{ "id": "head", "name": "Head", "image": "images/head.png",
                                "x": 0, "y": 0, "width": 10, "height": 10 }]
                }),
                vec![],
            ),
            (
                "garbage",
                serde_json::json!({
                    "format": "spriteloop.import", "version": 1,
                    "canvas": { "width": 100, "height": 100 },
                    "parts": [{ "id": "head", "name": "Head", "image": "images/head.png",
                                "x": 0, "y": 0, "width": 10, "height": 10 }]
                }),
                vec![("head.png", b"this is not an image".to_vec())],
            ),
            (
                "escape",
                serde_json::json!({
                    "format": "spriteloop.import", "version": 1,
                    "canvas": { "width": 100, "height": 100 },
                    "parts": [{ "id": "head", "name": "Head", "image": "../head.png",
                                "x": 0, "y": 0, "width": 10, "height": 10 }]
                }),
                vec![("head.png", png_bytes(10, 10))],
            ),
        ];
        for (tag, manifest, images) in cases {
            let mut core = core();
            let (assets0, sprites0, undo0) = (
                core.project.assets.len(),
                core.project.sprites.len(),
                core.undo_depth(),
            );
            let dir = temp_dir(tag);
            write_package(&dir, &manifest, &images);
            let err = import_layered_package(&mut core, &dir, 0)
                .expect_err("corrupt package must refuse");
            assert!(err.contains("refused"), "{tag} err: {err}");
            assert_eq!(core.project.assets.len(), assets0, "{tag}: no asset written");
            assert_eq!(core.project.sprites.len(), sprites0, "{tag}: no sprite written");
            assert!(
                core.project.figures[0].def.nodes.iter().all(|n| n.media.is_none()),
                "{tag}: no node dressed"
            );
            assert_eq!(core.undo_depth(), undo0, "{tag}: no undo step spent");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn folder_fallback_attaches_by_stem_and_centres_the_rest() {
        let mut core = core();
        let (sprites0, undo0) = (core.project.sprites.len(), core.undo_depth());
        let dir = temp_dir("folder");
        std::fs::write(dir.join("head.png"), png_bytes(30, 30)).unwrap();
        std::fs::write(dir.join("cape.png"), png_bytes(50, 50)).unwrap();
        std::fs::write(dir.join("notes.txt"), b"not an image").unwrap();
        let report = import_layered_folder(&mut core, &dir, 0).expect("folder imports");
        assert_eq!(report.attached, vec!["head"]);
        assert_eq!(report.sprites, vec!["cape"]);
        assert!(matches!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { .. })
        ));
        // No canvas geometry: the sprite lands at the stage
        // centre (480, 360) at scale 1.
        let key = core.project.sprites[sprites0].keys[0];
        assert_eq!((key.x, key.y), (480.0, 360.0));
        assert_eq!(key.scale, 1.0);
        assert_eq!(core.undo_depth(), undo0 + 1, "exactly one undo step");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invisible_part_degrades_to_a_transparent_sprite() {
        let mut core = core();
        let sprites0 = core.project.sprites.len();
        let dir = temp_dir("hidden");
        let manifest = serde_json::json!({
            "format": "spriteloop.import",
            "version": 1,
            "canvas": { "width": 960, "height": 720 },
            "parts": [
                { "id": "head", "name": "Head", "image": "images/head.png",
                  "x": 400, "y": 100, "width": 80, "height": 60, "visible": false }
            ]
        });
        write_package(&dir, &manifest, &[("head.png", png_bytes(80, 60))]);
        let report = import_layered_package(&mut core, &dir, 0).expect("package imports");
        // Even though "Head" names a node, an invisible part
        // is never attached (it would render anyway); it is
        // kept as an opacity-0 sprite — dropped nowhere.
        assert!(report.attached.is_empty());
        assert_eq!(report.sprites, vec!["Head"]);
        assert!(core.project.figures[0].def.nodes[2].media.is_none());
        assert_eq!(core.project.sprites[sprites0].keys[0].opacity, 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
