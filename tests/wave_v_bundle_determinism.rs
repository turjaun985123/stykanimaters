//! Wave V (V9) — `export_bundle` determinism: A-B-A
//! byte identity, member by member, over the bundle's
//! `images` kind on a POPULATED project.
//!
//! What the earlier suites pinned, and what this one
//! adds:
//!
//! - T8 (in-module, src/mcp_io.rs) pinned the bundle's
//!   scale routing on a cut-down bare demo: the PNG
//!   member honours `scale`, every other member is
//!   byte-unaffected, an invalid scale refuses before
//!   anything is written. U13/U15 carried refusal
//!   atomicity to the registry. S6/T9/U14 pinned A-B-A
//!   determinism on single funnels (raster, SVG) —
//!   never on the bundle as a whole.
//! - This suite drives the bundle through the real
//!   registry (`mcp::all_tools()`, the U13/U15 idiom)
//!   on U14's populated-project class of fixture — the
//!   intact demo figure WITH authored bends, rain
//!   weather, a credited two-line cue and a media
//!   sprite — and demands that two identical bundle
//!   exports agree on EVERY member (16 files: 5 PNG +
//!   5 JPEG + 5 SVG + the spritesheet), that an
//!   intervening DIFFERENT bundle provably differs,
//!   and that a return to the original project
//!   reproduces the first bundle byte for byte. Both
//!   scale classes are legged: unscaled, and `scale:
//!   2.0` (whose PNG member must double and whose
//!   other members must stay byte-identical to the
//!   unscaled bundle's — T8's routing contract, here
//!   on the populated fixture at registry level).
//!
//! The VIDEO kind is deliberately NOT legged, stated
//! plainly: `export_mp4`/`export_webm` shell out to an
//! external ffmpeg binary (a library test cannot
//! assume its presence or version), and the MP4
//! container carries encoder wall-clock metadata, so
//! byte identity across runs is not a property the
//! engine promises there. The images kind is pure
//! engine: every member writer renders from stored
//! project data in stored order and encodes in-process.
//!
//! Why the images bundle is deterministic BY
//! CONSTRUCTION (the legs are the proof, this is the
//! audit trail — every claim read at the source):
//! - Member writers are the plain sequence/sheet/SVG
//!   exports (src/export.rs, src/svg.rs): frames are
//!   written under index-derived names (`frame-{:04}`),
//!   renders are the pure funnels U14 audited (stored
//!   order, `prand` hash weather, no RNG state, no
//!   wall clock), and the `image` crate's PNG/JPEG
//!   encoders write no timestamps.
//! - No `SystemTime`/`Instant`/`HashMap` exists in
//!   src/export.rs or src/svg.rs (grep-verified); the
//!   bundle handler itself (src/mcp_io.rs) only names
//!   members after the project and dispatches.
//! - The comparison reads each bundle directory into
//!   a sorted (relative path, bytes) list — file
//!   mtimes and directory order never enter it.
//!
//! Everything writes inside per-test temp dirs; the
//! fixture sets fps 24.0 explicitly (board contract).

use std::io::Cursor;
use std::path::PathBuf;

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::all_tools;
use stykanimate_rs::model::{ImageAsset, InstanceKey, Project, SpriteInstance};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};
use stykanimate_rs::vfx::{Weather, WeatherKind};

/// The frame the fixture authors bends onto (the
/// demo's most displaced wave pose); bundle members
/// for it are the `frame-0003.*` files.
const FRAME: usize = 2;

/// The per-node bend (radians) authored onto every
/// non-root node at FRAME, and leg 1's deepened B
/// value (U14's constants).
const BEND_A: f32 = 0.35;
const BEND_B: f32 = 0.7;

// ---------------------------------------------------------------------
// Fixture — U14's populated project
// ---------------------------------------------------------------------

/// Solid-colour PNG bytes, encoded with the image
/// crate (encoder-made fixtures only).
fn png_bytes(w: u32, h: u32, px: image::Rgba<u8>) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    buf
}

/// Author bend data onto `frame` (U14's contract:
/// `frames[f].bends[figure][node]`, one inner Vec for
/// the single demo figure; the demo ships none).
fn author_bends(project: &mut Project, frame: usize, bend: f32) {
    let n = project.figures[0].def.nodes.len();
    assert!(n > 2, "the demo figure must have nodes to bend");
    let mut bends = vec![0.0f32; n];
    for b in bends.iter_mut().skip(1) {
        *b = bend;
    }
    project.frames[frame].bends = vec![bends];
}

/// The populated project, exactly U14's population
/// classes on the intact demo: authored bends, rain
/// (40 drops, wind 30), one credited TWO-LINE cue live
/// across the whole timeline (half-open [0, 5)) with
/// an explicit style, and one media sprite (encoder-
/// made 48x36 PNG at scale 2, centred at (200, 200),
/// its single key clamped from frame 0.0).
fn populated_fixture() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    assert_eq!(project.frames.len(), 5, "the demo ships five frames");
    assert_eq!(project.figures.len(), 1, "the demo ships one figure");

    author_bends(&mut project, FRAME, BEND_A);

    project.weather = Some(Weather {
        kind: WeatherKind::Rain,
        amount: 40.0,
        wind: 30.0,
    });

    project.subtitles = vec![SubtitleCue {
        start_frame: 0,
        end_frame: 5,
        text: "First line\nSecond line".to_string(),
        speaker: Some("MARA".to_string()),
    }];
    project.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Bottom,
        font_scale: 1.0,
        text_color: 0xFF_FF_FF,
        box_color: 0x00_00_00,
    };

    project.assets = vec![ImageAsset {
        name: "still.png".to_string(),
        png: png_bytes(48, 36, image::Rgba([200, 30, 30, 255])),
    }];
    project.sprites = vec![SpriteInstance {
        asset: 0,
        keys: vec![InstanceKey {
            frame: 0.0,
            x: 200.0,
            y: 200.0,
            scale: 2.0,
            angle: 0.0,
        }],
    }];
    project
}

// ---------------------------------------------------------------------
// Harness — the bundle through the real registry
// ---------------------------------------------------------------------

/// Invoke one tool's handler by name through the real
/// registry (the U13/U15 idiom).
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

/// A per-test temp directory (created empty); every
/// bundle a leg writes lives in its own subdirectory.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wave_v_bundle_determinism_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create per-test temp dir");
    dir
}

/// Run ONE `images` bundle of `project` into
/// `dir/tag` through the registry and return the
/// bundle directory. The reply is asserted whole —
/// all four members ok — so a silently failed member
/// can never masquerade as a deterministic empty one.
fn run_bundle(project: &Project, dir: &std::path::Path, tag: &str, scale: Option<f64>) -> PathBuf {
    let target = dir.join(tag);
    let mut core = EditorCore::new();
    core.project = project.clone();
    let mut args = json!({
        "kind": "images",
        "path": target.to_string_lossy(),
    });
    if let Some(s) = scale {
        args["scale"] = json!(s);
    }
    let reply = invoke(&mut core, "export_bundle", &args)
        .unwrap_or_else(|e| panic!("the {tag} bundle call must succeed: {e}"));
    assert_eq!(reply["ok"], json!(true), "{tag}: bundle reply not ok: {reply}");
    assert_eq!(
        reply["succeeded"],
        json!(4),
        "{tag}: all four image members must succeed: {reply}"
    );
    assert_eq!(
        reply["failed"],
        json!(0),
        "{tag}: no image member may fail: {reply}"
    );
    target
}

/// Every file under `dir` as (relative path, bytes),
/// sorted by path — the member-wise bundle comparison
/// (the mcp_io `bundle_tree` idiom).
fn read_tree(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("read bundle dir") {
            let p = entry.expect("bundle entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir)
                        .expect("member under the bundle dir")
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(&p).expect("read bundle member"),
                ));
            }
        }
    }
    out.sort();
    out
}

/// The 16 member names a 5-frame `images` bundle of
/// the demo must contain — the non-vacuity shape of
/// every tree this suite compares.
fn expected_member_names() -> Vec<String> {
    let mut names = Vec::new();
    for i in 1..=5 {
        names.push(format!("jpeg/frame-{i:04}.jpg"));
        names.push(format!("png/frame-{i:04}.png"));
        names.push(format!("svg/frame-{i:04}.svg"));
    }
    names.push("spritesheet.png".to_string());
    names.sort();
    names
}

fn member<'a>(tree: &'a [(String, Vec<u8>)], name: &str) -> &'a [u8] {
    tree.iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("bundle tree is missing member {name}"))
        .1
        .as_slice()
}

/// Member-by-member byte identity with a measured
/// difference on failure: the first differing member,
/// its first differing byte and differing-byte count
/// (the P47/S6 reporting discipline, one level up).
fn assert_trees_identical(a: &[(String, Vec<u8>)], b: &[(String, Vec<u8>)], context: &str) {
    let names_a: Vec<&String> = a.iter().map(|(n, _)| n).collect();
    let names_b: Vec<&String> = b.iter().map(|(n, _)| n).collect();
    assert_eq!(names_a, names_b, "{context}: member name sets differ");
    for ((name, ba), (_, bb)) in a.iter().zip(b.iter()) {
        if ba != bb {
            let shared = ba.len().min(bb.len());
            let first_diff = (0..shared).find(|&i| ba[i] != bb[i]);
            let differing =
                (0..shared).filter(|&i| ba[i] != bb[i]).count() + ba.len().abs_diff(bb.len());
            panic!(
                "{context}: member {name} differs — first differing byte at \
                 {first_diff:?}, {differing} bytes differ (lengths {} vs {})",
                ba.len(),
                bb.len()
            );
        }
    }
}

/// The names of the members whose bytes differ
/// between two trees with the same name set — leg
/// B's non-vacuity evidence, member-precise.
fn differing_members(a: &[(String, Vec<u8>)], b: &[(String, Vec<u8>)]) -> Vec<String> {
    let names_a: Vec<&String> = a.iter().map(|(n, _)| n).collect();
    let names_b: Vec<&String> = b.iter().map(|(n, _)| n).collect();
    assert_eq!(names_a, names_b, "compared bundles' member name sets differ");
    a.iter()
        .zip(b.iter())
        .filter(|((_, ba), (_, bb))| ba != bb)
        .map(|((n, _), _)| n.clone())
        .collect()
}

/// The (width, height) out of a PNG's IHDR header
/// (the mcp_io bundle-test idiom).
fn png_dims_of(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n", "not a PNG file");
    (
        u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width")),
        u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height")),
    )
}

/// A bundle tree must be exactly the expected 16
/// members, and its bent frame's SVG must carry the
/// credited cue's text — the population is IN the
/// bundle, not just in the fixture. (S12 pinned the
/// speaker prefix in the SVG `<text>`; this asserts
/// the bundle's SVG member kept it.)
fn assert_populated_tree(tree: &[(String, Vec<u8>)], context: &str) {
    let names: Vec<String> = tree.iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(
        names,
        expected_member_names(),
        "{context}: the bundle's member set is not the expected 16 files"
    );
    let svg = String::from_utf8_lossy(member(tree, "svg/frame-0003.svg"));
    assert!(
        svg.contains("MARA"),
        "{context}: the frame-0003 SVG member carries no 'MARA' credit — \
         the credited cue is not reaching the bundle"
    );
}

// ---------------------------------------------------------------------
// Leg 1 — unscaled A-B-A, member by member
// ---------------------------------------------------------------------

#[test]
fn bundle_images_unscaled_aba_is_byte_identical_member_by_member() {
    let dir = test_dir("unscaled");
    let base = populated_fixture();

    // A, then an immediate repeat into a second
    // directory: identical member by member.
    let a1 = read_tree(&run_bundle(&base, &dir, "a1", None));
    assert_populated_tree(&a1, "unscaled A");
    let a_repeat = read_tree(&run_bundle(&base, &dir, "a-repeat", None));
    assert_trees_identical(&a1, &a_repeat, "unscaled bundle exported twice back to back");

    // B: the bends deepened (0.35 -> 0.7 rad, U14's
    // interleave). The bent frame's raster members and
    // the sheet that tiles it must move.
    let mut bent = base.clone();
    author_bends(&mut bent, FRAME, BEND_B);
    let b = read_tree(&run_bundle(&bent, &dir, "b", None));
    let diffs = differing_members(&a1, &b);
    assert!(
        diffs.iter().any(|n| n == "png/frame-0003.png"),
        "deepening the bends changed no PNG member for the bent frame — \
         differing members: {diffs:?}"
    );
    assert!(
        diffs.iter().any(|n| n == "spritesheet.png"),
        "deepening the bends changed no spritesheet — the sheet is not \
         tiling the bent frame; differing members: {diffs:?}"
    );

    // Back to A: byte-identical to the first bundle,
    // no residue from the repeat or B.
    let a2 = read_tree(&run_bundle(&base, &dir, "a2", None));
    assert_trees_identical(
        &a1,
        &a2,
        "unscaled bundle A-B-A: first bundle vs the bundle after B",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — scaled (2.0) A-B-A + the cross-scale contract
// ---------------------------------------------------------------------

#[test]
fn bundle_images_scaled_aba_is_byte_identical_and_scales_only_the_png_member() {
    let dir = test_dir("scaled");
    let base = populated_fixture();

    // The unscaled reference bundle (for the
    // cross-scale assertions below).
    let unscaled = read_tree(&run_bundle(&base, &dir, "u-a", None));
    assert_populated_tree(&unscaled, "unscaled reference");

    // Scaled A, then an immediate repeat: identical
    // member by member.
    let a1 = read_tree(&run_bundle(&base, &dir, "s-a1", Some(2.0)));
    assert_populated_tree(&a1, "scaled A");
    let a_repeat = read_tree(&run_bundle(&base, &dir, "s-a-repeat", Some(2.0)));
    assert_trees_identical(&a1, &a_repeat, "scaled bundle exported twice back to back");

    // Cross-scale contract (T8's routing, populated +
    // registry level): the PNG member is a true 2.0
    // re-render — doubled IHDR dims, different bytes —
    // and EVERY other member is byte-identical to the
    // unscaled bundle's (the scale routes to the PNG
    // member only; SVG is scale-exempt by nature).
    assert_eq!(
        png_dims_of(member(&unscaled, "png/frame-0001.png")),
        (960, 720),
        "the unscaled PNG member keeps the demo stage size"
    );
    assert_eq!(
        png_dims_of(member(&a1, "png/frame-0001.png")),
        (1920, 1440),
        "the scaled PNG member doubles the demo stage size"
    );
    for (name, bytes) in &unscaled {
        if name.starts_with("png/") {
            assert_ne!(
                bytes.as_slice(),
                member(&a1, name),
                "PNG member {name} is byte-identical across scale classes — \
                 the scale is not reaching the PNG member"
            );
        } else {
            assert_eq!(
                bytes.as_slice(),
                member(&a1, name),
                "member {name} changed with the bundle scale — the scale \
                 must route to the PNG member only"
            );
        }
    }

    // B: the speaker renamed (MARA -> JUNIPER, U14's
    // scaled interleave). The composed prefix moves in
    // the scaled PNG burn-in AND in the SVG text.
    let mut renamed = base.clone();
    renamed.subtitles[0].speaker = Some("JUNIPER".to_string());
    let b = read_tree(&run_bundle(&renamed, &dir, "s-b", Some(2.0)));
    let diffs = differing_members(&a1, &b);
    assert!(
        diffs.iter().any(|n| n == "png/frame-0003.png"),
        "renaming the speaker changed no scaled PNG member — the scaled \
         burn-in is not reaching the bundle; differing members: {diffs:?}"
    );
    assert!(
        diffs.iter().any(|n| n == "svg/frame-0003.svg"),
        "renaming the speaker changed no SVG member — the credited text \
         is not reaching the bundle; differing members: {diffs:?}"
    );

    // Back to A: byte-identical, no residue.
    let a2 = read_tree(&run_bundle(&base, &dir, "s-a2", Some(2.0)));
    assert_trees_identical(
        &a1,
        &a2,
        "scaled bundle A-B-A: first bundle vs the bundle after B",
    );

    let _ = std::fs::remove_dir_all(&dir);
}
