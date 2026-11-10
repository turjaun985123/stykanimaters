//! Wave S (S10) — JPEG parity end to end: a JPEG asset
//! living in a project through the whole stack the PNG
//! control beside it travels — the content-magic import
//! funnel (Wave R, R9), the project save/load round
//! trip, the raster media composite, and the SVG writer
//! (whose embeds label bytes by sniffed MIME since
//! IA-R9-3).
//!
//! R18 pinned the first hop (import + a sprite render at
//! op level). What it did not pin, and this file does:
//!
//! - the JPEG's stored bytes and dimensions survive a
//!   save→load round trip VERBATIM, sprite instance
//!   intact, next to a PNG control asset in the same
//!   project;
//! - after that round trip the JPEG's pixels still
//!   reach a rendered frame through the raster
//!   composite;
//! - the SVG writer embeds the JPEG asset under
//!   `data:image/jpeg;base64,` and the PNG asset under
//!   `data:image/png;base64,` in the same export, each
//!   payload decoding to exactly the stored bytes;
//! - JPEG magic over a garbage body is still refused
//!   end to end: nothing stored, no undo step spent,
//!   and nothing reaches the saved project either.
//!
//! Tolerance rationale (leg 2): JPEG is a LOSSY codec —
//! 8x8 DCT quantisation (and chroma subsampling) means a
//! decoded solid field is close to, not equal to, the
//! colour that was encoded; the `image` crate's encoder
//! makes no exactness promise. So the JPEG leg asserts
//! the rendered centre pixel within ±16 per channel
//! (~6% of full scale) of the source colour — far
//! tighter than the distance to the stage background or
//! the PNG control's colour, so the leg still proves
//! the JPEG's colour is what was painted — while the
//! PNG control leg asserts EXACT equality, because PNG
//! is lossless and the composite stamps it at alpha
//! 1.0. Asserting exactness for the JPEG would pin a
//! property the format does not have.
//!
//! Fixtures set fps explicitly (24.0) per the wave's
//! timing contract; every test builds its own fixtures
//! and its own temp directory.

use std::io::Cursor;
use std::path::PathBuf;

use serde_json::json;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export::render_frame;
use stykanimate_rs::media::{png_dims, tool_asset_import};
use stykanimate_rs::model::{Frame, InstanceKey, Project, SpriteInstance};
use stykanimate_rs::projects;
use stykanimate_rs::svg::export_svg;

/// Per-channel tolerance for JPEG-decoded colour: JPEG
/// is lossy (see the module docs), PNG is not — the PNG
/// legs assert exact values and never use this.
const JPEG_TOL: u8 = 16;

const JPEG_RGB: [u8; 3] = [200, 30, 30];
const PNG_RGB: [u8; 3] = [0, 200, 0];
const BG_RGB: [u8; 3] = [16, 16, 16];

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// Solid-colour JPEG bytes, encoded with the image
/// crate exactly as the media module's own fixtures
/// build them.
fn jpeg_bytes(w: u32, h: u32, px: image::Rgb<u8>) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .expect("encode JPEG fixture");
    buf
}

/// Solid-colour PNG bytes (the control format).
fn png_bytes(w: u32, h: u32, px: image::Rgba<u8>) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    buf
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-s-jpeg-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// A core on a bare 200x150 stage (background 0x101010,
/// no figures, sprites, texts, strokes, subtitles or
/// assets, one empty frame), at 24 fps — so the only
/// paint a render can show is the media placed by the
/// test itself.
fn bare_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core.project.stage = [200.0, 150.0];
    core.project.background = 0x101010;
    core.project.figures.clear();
    core.project.frames = vec![Frame {
        poses: Vec::new(),
        duration: 1.0,
        easing: None,
        bends: Vec::new(),
    }];
    core.project.sprites.clear();
    core.project.texts.clear();
    core.project.strokes.clear();
    core.project.subtitles.clear();
    core.project.assets.clear();
    core
}

/// Write `bytes` to `dir/name` and import the file
/// through the asset funnel; returns the tool's reply.
fn import_file(core: &mut EditorCore, dir: &PathBuf, name: &str, bytes: &[u8]) -> serde_json::Value {
    let file = dir.join(name);
    std::fs::write(&file, bytes).expect("write import file");
    tool_asset_import(core, &json!({ "path": file.to_string_lossy() }))
        .expect("the fixture file imports through the funnel")
}

/// A sprite instance of `asset` centred at (x, y) from
/// frame 0 at the given scale, unrotated.
fn sprite_at(asset: usize, x: f32, y: f32, scale: f32) -> SpriteInstance {
    SpriteInstance {
        asset,
        keys: vec![InstanceKey {
            frame: 0.0,
            x,
            y,
            scale,
            angle: 0.0,
        }],
    }
}

/// The dual-asset fixture: a 24x18 JPEG and a 20x20 PNG
/// imported into a bare core (the JPEG from a file whose
/// name does not say JPEG — content decides), each
/// instanced as a sprite at scale 2 on the stage: the
/// JPEG's 48x36 quad centred at (60, 75), the PNG's
/// 40x40 quad centred at (150, 75). Returns the core,
/// the two asset indices and the two stored byte
/// strings.
fn dual_asset_core(dir: &PathBuf) -> (EditorCore, usize, usize, Vec<u8>, Vec<u8>) {
    let mut core = bare_core();
    let jpeg = jpeg_bytes(24, 18, image::Rgb(JPEG_RGB));
    assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]), "fixture is a real JPEG");
    let out = import_file(&mut core, dir, "photo.bin", &jpeg);
    let jpeg_idx = out["index"].as_u64().expect("JPEG import returns an index") as usize;
    assert_eq!(out["width"].as_u64(), Some(24));
    assert_eq!(out["height"].as_u64(), Some(18));

    let png = png_bytes(20, 20, image::Rgba([PNG_RGB[0], PNG_RGB[1], PNG_RGB[2], 255]));
    let out = import_file(&mut core, dir, "still.png", &png);
    let png_idx = out["index"].as_u64().expect("PNG import returns an index") as usize;
    assert_eq!(out["width"].as_u64(), Some(20));
    assert_eq!(out["height"].as_u64(), Some(20));

    core.project.sprites.push(sprite_at(jpeg_idx, 60.0, 75.0, 2.0));
    core.project.sprites.push(sprite_at(png_idx, 150.0, 75.0, 2.0));
    (core, jpeg_idx, png_idx, jpeg, png)
}

/// Save `project` through the project funnel and load
/// it straight back.
fn save_load(project: &Project, dir: &PathBuf) -> Project {
    let path = projects::save(project, dir).expect("save fixture project");
    projects::load(&path).expect("load fixture project")
}

/// Pixels within `tol` of `target` on every channel.
fn count_near(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    target: [u8; 3],
    tol: u8,
) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y).0;
            if p.iter().zip(target).all(|(a, b)| a.abs_diff(b) <= tol) {
                n += 1;
            }
        }
    }
    n
}

// ---------------------------------------------------------------------
// Leg 1 — the JPEG lives through save→load, verbatim
// ---------------------------------------------------------------------

#[test]
fn s10_jpeg_asset_survives_save_load_verbatim_with_dims_and_instance() {
    let dir = temp_dir("round-trip");
    let (core, jpeg_idx, png_idx, jpeg, png) = dual_asset_core(&dir);

    // Pre-save truth: the funnel stored both files'
    // bytes verbatim, and the content-sniffed dims agree.
    assert_eq!(core.project.assets[jpeg_idx].png, jpeg);
    assert_eq!(core.project.assets[png_idx].png, png);
    assert_eq!(png_dims(&core.project, jpeg_idx), Some((24, 18)));
    assert_eq!(png_dims(&core.project, png_idx), Some((20, 20)));

    let loaded = save_load(&core.project, &dir.join("save"));

    // The stored bytes are the file's bytes, verbatim,
    // for the JPEG exactly as for the PNG control — the
    // project file is a byte array either way, and the
    // JPEG must not be re-encoded, transcoded or dropped
    // anywhere on the path.
    assert_eq!(loaded.assets.len(), 2, "both assets survive the round trip");
    assert_eq!(
        loaded.assets[jpeg_idx].png, jpeg,
        "the JPEG's stored bytes must survive save→load verbatim"
    );
    assert_eq!(
        loaded.assets[png_idx].png, png,
        "the PNG control's stored bytes must survive save→load verbatim"
    );
    assert_eq!(
        png_dims(&loaded, jpeg_idx),
        Some((24, 18)),
        "the loaded JPEG still decodes at its imported dimensions"
    );
    assert_eq!(png_dims(&loaded, png_idx), Some((20, 20)));

    // The instances that attach the assets survive too,
    // still naming the same asset indices.
    assert_eq!(loaded.sprites.len(), 2);
    assert_eq!(loaded.sprites[0].asset, jpeg_idx);
    assert_eq!(loaded.sprites[1].asset, png_idx);

    // And the whole document is a fixed point of
    // save→load (the P22 discipline): re-serialising the
    // loaded project reproduces the original's
    // serialisation byte for byte.
    assert_eq!(
        serde_json::to_string_pretty(&loaded).expect("re-serialise loaded project"),
        serde_json::to_string_pretty(&core.project).expect("serialise original project"),
        "save→load must be byte-identical for the dual-asset project"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — the round-tripped JPEG reaches a rendered frame
// ---------------------------------------------------------------------

#[test]
fn s10_jpeg_pixels_reach_a_rendered_frame_after_round_trip_png_control_exact() {
    let dir = temp_dir("raster");
    let (core, _, _, _, _) = dual_asset_core(&dir);
    let loaded = save_load(&core.project, &dir.join("save"));

    let img = render_frame(&loaded, 0);
    assert_eq!((img.width(), img.height()), (200, 150));

    // The JPEG quad's centre carries the JPEG's colour,
    // within the lossy tolerance (module docs): JPEG
    // quantises, so closeness — not equality — is the
    // honest claim.
    let centre = img.get_pixel(60, 75).0;
    for (channel, want) in centre.iter().zip(JPEG_RGB) {
        assert!(
            channel.abs_diff(want) <= JPEG_TOL,
            "JPEG centre pixel {centre:?} must sit within ±{JPEG_TOL} of {JPEG_RGB:?} per channel"
        );
    }
    // And the colour covers the quad, not just a point:
    // the 48x36 quad's interior is 1,728 px, nearly all
    // of it within tolerance of the source colour (edge
    // ringing is the only honest exception).
    let covered = count_near(&img, JPEG_RGB, JPEG_TOL);
    assert!(
        covered >= 800,
        "only {covered} px within ±{JPEG_TOL} of the JPEG colour — the quad must paint broadly"
    );

    // The PNG control is lossless and composited at
    // alpha 1.0: its centre is EXACTLY its colour.
    assert_eq!(
        img.get_pixel(150, 75).0,
        PNG_RGB,
        "the PNG control's centre must be exact (PNG is lossless)"
    );
    // Outside both quads the stage background is exact
    // and untouched.
    assert_eq!(img.get_pixel(4, 4).0, BG_RGB);
    assert_eq!(img.get_pixel(195, 145).0, BG_RGB);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — the SVG writer labels each embed by its bytes
// ---------------------------------------------------------------------

/// Decode a standard base64 payload (the SVG writer's
/// embed encoding) back to bytes.
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(b: u8) -> u32 {
        match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a') as u32 + 26,
            b'0'..=b'9' => (b - b'0') as u32 + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let has2 = chunk.len() > 2 && chunk[2] != b'=';
        let has3 = chunk.len() > 3 && chunk[3] != b'=';
        let n = (val(chunk[0]) << 18)
            | (val(chunk[1]) << 12)
            | if has2 { val(chunk[2]) << 6 } else { 0 }
            | if has3 { val(chunk[3]) } else { 0 };
        out.push((n >> 16) as u8);
        if has2 {
            out.push((n >> 8) as u8);
        }
        if has3 {
            out.push(n as u8);
        }
    }
    out
}

/// The payload of the (single) `data:<mime>;base64,…`
/// embed in an SVG document.
fn embed_payload<'a>(svg: &'a str, mime: &str) -> &'a str {
    let marker = format!("data:{mime};base64,");
    let start = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("the SVG carries no {mime} embed:\n{svg}"))
        + marker.len();
    let end = svg[start..]
        .find('"')
        .map(|i| start + i)
        .expect("the embed payload is quoted");
    &svg[start..end]
}

#[test]
fn s10_svg_embeds_jpeg_as_jpeg_and_png_as_png_with_the_stored_bytes() {
    let dir = temp_dir("svg");
    let (core, _, _, jpeg, png) = dual_asset_core(&dir);

    let svg_dir = dir.join("svg-out");
    let n = export_svg(&core.project, &svg_dir).expect("the SVG export writes");
    assert_eq!(n, 1, "one authored frame exports one SVG");
    let svg = std::fs::read_to_string(svg_dir.join("frame-0001.svg")).expect("read the frame SVG");

    // Each asset is embedded exactly once, under the
    // MIME its own bytes sniff as (IA-R9-3): the JPEG is
    // NOT mislabelled as PNG, and the PNG control is not
    // swept into the JPEG label either.
    assert_eq!(
        svg.matches("data:image/jpeg;base64,").count(),
        1,
        "exactly one JPEG embed, labelled image/jpeg"
    );
    assert_eq!(
        svg.matches("data:image/png;base64,").count(),
        1,
        "exactly one PNG embed, labelled image/png"
    );

    // The payloads are the stored bytes themselves,
    // byte for byte — the embed is a transport of the
    // asset table, not a re-encode.
    assert_eq!(
        base64_decode(embed_payload(&svg, "image/jpeg")),
        jpeg,
        "the JPEG embed's payload must decode to the stored JPEG bytes"
    );
    assert_eq!(
        base64_decode(embed_payload(&svg, "image/png")),
        png,
        "the PNG embed's payload must decode to the stored PNG bytes"
    );

    // The classic magic prefixes ride along as a
    // cross-check: a JPEG payload starts /9j/ (FF D8 FF),
    // a PNG payload iVBORw0KGgo (the PNG signature).
    assert!(
        embed_payload(&svg, "image/jpeg").starts_with("/9j/"),
        "the JPEG payload carries the JPEG magic prefix"
    );
    assert!(
        embed_payload(&svg, "image/png").starts_with("iVBORw0KGgo"),
        "the PNG payload carries the PNG magic prefix"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 4 — JPEG magic over garbage is refused end to end
// ---------------------------------------------------------------------

#[test]
fn s10_jpeg_magic_over_garbage_is_refused_and_nothing_persists() {
    let dir = temp_dir("refusal");
    let mut core = bare_core();
    let depth = core.undo_depth();

    // A JPEG SOI over a garbage body passes the magic
    // gate and must be refused at decode — the funnel's
    // second line of defence.
    let mut junk = vec![0xFF, 0xD8, 0xFF];
    junk.extend_from_slice(b"garbagegarbage");
    let junk_file = dir.join("junk.jpg");
    std::fs::write(&junk_file, &junk).expect("write junk file");
    let err = tool_asset_import(&mut core, &json!({ "path": junk_file.to_string_lossy() }))
        .expect_err("JPEG magic over garbage must be refused");
    assert!(err.contains("does not decode"), "err: {err}");

    // End to end: the refusal leaves no trace anywhere —
    // no asset in the table, no undo step spent, and a
    // save→load of the project still holds no asset (the
    // refusal cannot leak into persistence).
    assert!(core.project.assets.is_empty(), "a refused import stores no asset");
    assert_eq!(core.undo_depth(), depth, "a refused import spends no undo step");
    let loaded = save_load(&core.project, &dir.join("save"));
    assert!(
        loaded.assets.is_empty(),
        "the saved project must not contain the refused asset either"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
