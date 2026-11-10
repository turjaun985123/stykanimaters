//! Wave Y (Y6) — the MCP handler for the layered-set
//! import (SpriteLoop GAP P-9's registry surface).
//!
//! The importer itself is Y5's `crate::layered_import`:
//! free functions over `EditorCore` that bring a
//! SpriteLoop package folder (a `spriteloop.import.json`
//! plus one cropped image per part, as the official
//! Krita/Photoshop exporters write it) — or a plain
//! folder of PNG/JPEG images — into the current project
//! as dressing for one figure. Parts matched by name to
//! the figure's nodes become segment media on those
//! nodes; every unmatched or invisible part becomes a
//! free sprite. Layers dress stick nodes; they never
//! build the character.
//!
//! This module is the thin verified shell over that
//! importer, in the sprite-handler family's shape
//! (`mcp_sprite_tools`): local `lrd_` argument helpers,
//! the importer's own refusals surfaced verbatim (it
//! owns every filesystem and validation refusal, in its
//! "layered import refused: …" voice — a second refusal
//! voice here would only drift), and the effect verified
//! from the resulting project state before success is
//! reported. The importer's discipline is the shell's
//! guarantee: every part validated through the shared
//! asset gate (`media::asset_from_bytes`) before any
//! write, one corrupt part refuses the whole import, a
//! success spends exactly one undo step.
//!
//! The handler carries its `// TOOL ENTRY SPEC:` (the
//! registry entry for `mcp_tools::tools()` plus its
//! MCP.md catalogue row) for the integrator to splice
//! verbatim; the module declaration in `lib.rs` is
//! likewise an integrator action (see the Y6 findings).

use std::path::Path;

use crate::editor::EditorCore;
use serde_json::{json, Value};

// --------------------------------------------------------------------
// Argument helpers (local duplicates of the media.rs
// `ast_` shapes — see the module docs).
// --------------------------------------------------------------------

/// One argument of the MCP call (the `ast_get` shape,
/// media.rs).
fn lrd_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.as_object().and_then(|o| o.get(key))
}

fn lrd_as_usize(v: &Value, key: &str) -> Result<usize, String> {
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

/// Required string argument (the `ast_req_str` shape,
/// media.rs).
fn lrd_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match lrd_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Optional non-negative integer argument: absent is
/// `None`, present-but-not-an-integer refuses in the
/// family wording (never a silent default).
fn lrd_opt_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match lrd_get(args, key) {
        Some(v) => lrd_as_usize(v, key).map(Some),
        None => Ok(None),
    }
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the sprite block immediately after the
// sprite_reorder entry (before the Wave 14 parity-fill
// banner).
//   Tool {
//       name: "import_layered_set",
//       description: "Import a layered set into the current project from the folder at 'path' — a SpriteLoop package folder (holding spriteloop.import.json, as the official Krita/Photoshop exporters write it) or a plain folder of PNG/JPEG images; a manifest in the folder always wins over the loose-images reading. Every part's image is validated through the shared asset gate before anything changes, so one corrupt part refuses the whole import and spends no undo step. Parts are matched by name to nodes of figure 'figure' (the selected figure when 'figure' is omitted): a matched part dresses its node as segment media, and every unmatched or invisible part becomes a free sprite — no part is ever dropped. The whole import is one undo step.",
//       schema: obj_schema(
//           json!({
//               "path": { "type": "string" },
//               "figure": { "type": "integer" }
//           }),
//           &["path"],
//       ),
//       handler: crate::mcp_layered_tools::tool_import_layered_set,
//   }
// MCP.md row (insert after the `sprite_reorder` row):
//   | `import_layered_set` | Import a layered set into the current project from the folder at 'path' — a SpriteLoop package folder (holding spriteloop.import.json, as the official Krita/Photoshop exporters write it) or a plain folder of PNG/JPEG images; a manifest in the folder always wins over the loose-images reading. Every part's image is validated through the shared asset gate before anything changes, so one corrupt part refuses the whole import and spends no undo step. Parts are matched by name to nodes of figure 'figure' (the selected figure when 'figure' is omitted): a matched part dresses its node as segment media, and every unmatched or invisible part becomes a free sprite — no part is ever dropped. The whole import is one undo step. (Wave Y, Y6) | `path` | `figure` |
/// MCP handler for `import_layered_set`: import the
/// layered set rooted at `path` via
/// `layered_import::import_layered_folder` (which
/// delegates to the package reading when the folder
/// holds a manifest). `figure` names the figure to
/// dress and defaults to the selected figure — the GUI
/// command's target — when omitted; the importer's own
/// range check refuses a bad index in its wording. The
/// path is deliberately NOT pre-checked here: every
/// filesystem refusal (missing folder, unreadable
/// manifest, an image path escaping the package, a
/// corrupt image at the asset gate) is the importer's,
/// in its own voice, and arrives before any write. The
/// result is verified from the resulting state — the
/// asset table grew by the reported count, the sprite
/// table by the reported sprite count, and attached +
/// sprites accounts for every part — before success is
/// reported.
pub fn tool_import_layered_set(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = lrd_req_str(args, "path")?;
    let figure = lrd_opt_usize(args, "figure")?.unwrap_or(core.selected_figure);
    let assets_before = core.project.assets.len();
    let sprites_before = core.project.sprites.len();
    let report = crate::layered_import::import_layered_folder(core, Path::new(path), figure)?;
    let verified = core.project.assets.len() - assets_before == report.assets_added
        && core.project.sprites.len() - sprites_before == report.sprites.len()
        && report.attached.len() + report.sprites.len() == report.parts;
    if verified {
        Ok(json!({
            "ok": true,
            "path": path,
            "figure": figure,
            "parts": report.parts,
            "assets_added": report.assets_added,
            "attached": report.attached,
            "sprites": report.sprites,
            "attached_opacity_stored": report.attached_opacity_stored,
            "status": core.status,
        }))
    } else {
        Err(core.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::path::PathBuf;

    /// Encoder-made PNG bytes (the Y5 fixture pattern —
    /// never hand-packed image bytes).
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([40, 120, 200, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-layered-mcp-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A demo-project core set up by direct field write,
    /// so the undo depth starts at exactly 0 (the
    /// sprite-fixture pattern).
    fn core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project = Project::new_demo();
        core
    }

    /// The whole project as its snapshot JSON — the
    /// exact bytes `snapshot()` would store. `Project`
    /// has no `PartialEq`, so this is how "untouched" is
    /// asserted (the `sprite_ops` pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn folder_import_reports_parts_and_spends_one_undo_step() {
        let mut core = core();
        let (assets0, sprites0, undo0) = (
            core.project.assets.len(),
            core.project.sprites.len(),
            core.undo_depth(),
        );
        let dir = temp_dir("folder");
        // Stems that slug-match no demo node: both parts
        // must degrade to free sprites, never drop.
        std::fs::write(dir.join("y6alpha.png"), png_bytes(8, 8)).unwrap();
        std::fs::write(dir.join("y6beta.png"), png_bytes(6, 10)).unwrap();
        let out = tool_import_layered_set(
            &mut core,
            &json!({ "path": dir.to_string_lossy(), "figure": 0 }),
        )
        .expect("folder import succeeds");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["parts"], json!(2));
        assert_eq!(out["assets_added"], json!(2));
        assert_eq!(out["attached"], json!([]));
        assert_eq!(out["sprites"], json!(["y6alpha", "y6beta"]));
        assert_eq!(core.project.assets.len(), assets0 + 2);
        assert_eq!(core.project.sprites.len(), sprites0 + 2);
        assert_eq!(
            core.undo_depth(),
            undo0 + 1,
            "the whole import is exactly one undo step"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_import_attaches_the_matching_part_by_name() {
        let mut core = core();
        let undo0 = core.undo_depth();
        let dir = temp_dir("package");
        std::fs::create_dir_all(dir.join("images")).unwrap();
        let manifest = json!({
            "format": "spriteloop.import",
            "version": 1,
            "canvas": { "width": 960, "height": 720 },
            "parts": [
                { "id": "head", "name": "Head", "image": "images/head.png",
                  "x": 400, "y": 100, "width": 80, "height": 60 },
                { "id": "cape", "name": "Cape", "image": "images/cape.png",
                  "x": 100, "y": 100, "width": 50, "height": 50 }
            ]
        });
        std::fs::write(
            dir.join("spriteloop.import.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("images").join("head.png"), png_bytes(80, 60)).unwrap();
        std::fs::write(dir.join("images").join("cape.png"), png_bytes(50, 50)).unwrap();
        // No `figure` argument: the selected figure (0)
        // is dressed, as the GUI command dresses it.
        let out = tool_import_layered_set(&mut core, &json!({ "path": dir.to_string_lossy() }))
            .expect("package import succeeds");
        assert_eq!(out["figure"], json!(0));
        assert_eq!(out["parts"], json!(2));
        assert_eq!(out["attached"], json!(["Head"]));
        assert_eq!(out["sprites"], json!(["Cape"]));
        assert_eq!(core.undo_depth(), undo0 + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_and_mistyped_arguments_refuse_spending_nothing() {
        let mut core = core();
        let before = project_json(&core);
        let err = tool_import_layered_set(&mut core, &json!({})).unwrap_err();
        assert_eq!(err, "missing required argument 'path'");
        let err = tool_import_layered_set(&mut core, &json!({ "path": 3 })).unwrap_err();
        assert_eq!(err, "argument 'path' must be a string");
        let err = tool_import_layered_set(
            &mut core,
            &json!({ "path": "/nonexistent-y6", "figure": -1 }),
        )
        .unwrap_err();
        assert_eq!(err, "argument 'figure' must be a non-negative integer");
        assert_eq!(project_json(&core), before, "refusals change nothing");
        assert_eq!(core.undo_depth(), 0, "refusals spend no undo step");
    }

    #[test]
    fn corrupt_part_refuses_the_whole_import_spending_nothing() {
        let mut core = core();
        let before = project_json(&core);
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join("good.png"), png_bytes(8, 8)).unwrap();
        std::fs::write(dir.join("bad.png"), b"not an image at all").unwrap();
        let err = tool_import_layered_set(
            &mut core,
            &json!({ "path": dir.to_string_lossy(), "figure": 0 }),
        )
        .unwrap_err();
        assert!(
            err.contains("layered import refused"),
            "the importer's refusal surfaces verbatim: {err}"
        );
        assert_eq!(project_json(&core), before, "no part of the set lands");
        assert_eq!(core.undo_depth(), 0, "a refused import spends no undo step");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
