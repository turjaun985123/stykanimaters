//! Wave V (V10) — cross-feature e2e: three seams the
//! earlier waves pinned only in pieces.
//!
//! * Leg 1 — the TOP subtitle anchor, raster-proven at
//!   core level. The Bottom anchor's raster geometry is
//!   covered (Wave P's in-crate burn-in pins, U15's
//!   scaled-JPEG band), and Top was exercised by the
//!   headless census and in-crate band pins — but no
//!   integration leg measured a Top cue's painted face
//!   against the shared layout helpers
//!   (`subtitle_line_center_y`, `subtitle_stack_line_offset`)
//!   through the public render funnel. Leg 1 renders
//!   through `export::render_frame` and derives every
//!   expectation from those helpers at run time: a
//!   single cue at the edge band, a two-cue stack in
//!   cue order, a multiline cue whose lines read AWAY
//!   from the top edge in their own order (the Bottom
//!   multiline pin's mirror image), and half-open
//!   liveness at the top anchor.
//!
//!   The face-extent model is the in-crate derivation
//!   (export.rs's burn-in pins, CI-proven): at the
//!   default style the 3x5 stamp's cell is 4 stage
//!   units, and a line's text-colour face spans
//!   [centre − 8, centre + 11] rows about the helper's
//!   line centre — centre of the span therefore sits
//!   1.5 rows below the helper centre. Assertions use
//!   that span with a few rows of slack for glyph-row
//!   variance, never a hardcoded band.
//!
//! * Leg 2 — the JPEG chain, import to scaled export.
//!   S10 proved a JPEG asset survives save→load and
//!   reaches a 1.0 render next to a PNG control; U15
//!   proved scaled sequences carry subtitle burn-in.
//!   Neither instanced a JPEG through the registry and
//!   measured it in a SCALED sequence export. Leg 2
//!   imports an encoder-made JPEG through
//!   `asset_import`, instances it with `sprite_add`,
//!   exports the PNG sequence at 2.0, and measures the
//!   decoded frames: the sprite's quad doubles with
//!   the picture, centred where the instance key put
//!   it, in the JPEG's colour (within the lossy
//!   tolerance S10 set — a solid-field JPEG decodes
//!   near-exactly, and the PNG transport adds no loss).
//!
//! * Leg 3 — figure-library name fidelity, written
//!   against V1's board contract. The defect (proven
//!   by CI round 5 of the Wave R line): saving a
//!   figure named "Styk" wrote `styk.json`, and the
//!   library — which names entries by file stem —
//!   listed and inserted it as "styk". V1 runs
//!   concurrently with this file; its contract is
//!   that a saved figure's display name round-trips
//!   EXACTLY (save "Styk" → listed as "Styk" →
//!   insert by that name yields a figure named
//!   "Styk"), and that libraries already on disk in
//!   lower-case form keep loading. The first leg-3
//!   test asserts the round-trip contract through
//!   the registry; on the pre-V1 tree it fails on the
//!   case assertion — that failure IS the defect.
//!   The second test is the migration guard: a
//!   hand-planted lower-case `styk.json` in the
//!   current on-disk format still lists and inserts,
//!   under its stem name, before and after the fix.
//!
//!   The library folder is `./library` relative to
//!   the process current directory, so the leg-3
//!   tests enter their own temp dirs serialised on a
//!   mutex (the wave_p_figlib discipline) and restore
//!   the directory on drop.
//!
//! Fixtures set fps explicitly (24.0) per the wave's
//! timing contract; image fixtures are encoder-made
//! (the `image` crate), never hand-rolled bytes.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export::{export_png_sequence, export_png_sequence_scaled, render_frame};
use stykanimate_rs::mcp::all_tools;
use stykanimate_rs::model::{FigureDef, Frame, Project};
use stykanimate_rs::subtitles::{
    subtitle_line_center_y, subtitle_line_height, SubtitleCue, SubtitlePosition, SubtitleStyle,
};

// ---------------------------------------------------------------------
// Shared fixtures and helpers
// ---------------------------------------------------------------------

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-v-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Invoke one tool's handler by name through the real
/// registry (`mcp::all_tools()`), the undo census's
/// idiom.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

type Rgb8 = image::ImageBuffer<image::Rgb<u8>, Vec<u8>>;

/// The subtitle text-face predicate: the default
/// style's text colour is 0xFFFFFF and the burn-in's
/// centre stamp paints it exactly; the backing is
/// black and the fixture background 0x333333, both far
/// below the threshold (U15's predicate).
fn is_face_white(px: [u8; 3]) -> bool {
    px[0] >= 240 && px[1] >= 240 && px[2] >= 240
}

/// Rows of a frame holding face ink.
fn rows_with_face(img: &Rgb8) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| is_face_white(img.get_pixel(x, y).0)) {
            rows.push(y);
        }
    }
    rows
}

/// How many face-ink pixels a frame holds.
fn count_face(img: &Rgb8) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                n += 1;
            }
        }
    }
    n
}

/// How many face-ink pixels sit in one row range.
fn count_face_in_rows(img: &Rgb8, rows: std::ops::RangeInclusive<u32>) -> usize {
    let mut n = 0;
    for y in rows {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                n += 1;
            }
        }
    }
    n
}

/// Group sorted rows into runs of consecutive rows.
fn row_clusters(rows: &[u32]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &y in rows {
        match out.last_mut() {
            Some((_, last)) if *last + 1 == y => *last = y,
            _ => out.push((y, y)),
        }
    }
    out
}

/// Centre of a row cluster.
fn cluster_centre(cluster: (u32, u32)) -> f32 {
    (cluster.0 + cluster.1) as f32 / 2.0
}

// ---------------------------------------------------------------------
// Leg 1 — the Top anchor, measured against the
// shared layout helpers
// ---------------------------------------------------------------------

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

/// An empty 480x400 stage (background 0x333333, no
/// figures) carrying exactly `cues` under `style`, at
/// 24 fps — the burn-in pins' fixture shape, rebuilt
/// here so the only paint a render can show is the
/// burn-in itself.
fn staged_project(frames: usize, cues: Vec<SubtitleCue>, style: SubtitleStyle) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = [480.0, 400.0];
    project.background = 0x333333;
    project.figures.clear();
    project.frames = (0..frames)
        .map(|_| Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    project.subtitles = cues;
    project.subtitle_style = style;
    project
}

fn top_style() -> SubtitleStyle {
    SubtitleStyle {
        position: SubtitlePosition::Top,
        ..SubtitleStyle::default()
    }
}

/// Assert one face cluster is the line the helpers
/// place `from_edge` lines from `style`'s anchor edge:
/// inside the line extent about the helper's line centre
/// ([centre − 10, centre + 13] at the default style,
/// with slack), spanning close to the shaped cap-ink
/// (~14 rows at scale 1), its own centre within a few
/// rows of the span's centre (helper centre + 1.5).
fn assert_cluster_at_helper_line(
    cluster: (u32, u32),
    style: SubtitleStyle,
    stage_height: f32,
    from_edge: usize,
    what: &str,
) {
    let centre = subtitle_line_center_y(style, stage_height, from_edge);
    let (lo, hi) = cluster;
    assert!(
        lo as f32 >= centre - 10.0 && hi as f32 <= centre + 13.0,
        "{what}: face rows {lo}..={hi} must sit inside the line extent about \
         the helper centre {centre} (from_edge {from_edge})"
    );
    let span = hi - lo + 1;
    assert!(
        (12..=17).contains(&span),
        "{what}: face spans {span} rows, expected the ~14-row shaped cap-ink"
    );
    let measured = cluster_centre(cluster);
    assert!(
        (measured - (centre + 1.5)).abs() <= 3.5,
        "{what}: face centre {measured} must sit at the helper centre {centre} \
         plus the stamp's 1.5-row offset"
    );
}

#[test]
fn v10_top_anchor_single_cue_matches_the_shared_layout_helpers() {
    let style = top_style();
    let project = staged_project(1, vec![cue(0, 30, "HELLO")], style);
    let img = render_frame(&project, 0);
    assert_eq!((img.width(), img.height()), (480, 400));

    let clusters = row_clusters(&rows_with_face(&img));
    assert_eq!(
        clusters.len(),
        1,
        "a single top cue paints exactly one face cluster: {clusters:?}"
    );
    assert_cluster_at_helper_line(clusters[0], style, 400.0, 0, "top single cue");

    // The anchor really is the top: no face ink in the
    // frame's bottom half at all.
    assert_eq!(
        count_face_in_rows(&img, 200..=399),
        0,
        "a top-anchored cue paints nothing in the bottom half"
    );

    // The comparative half: the SAME fixture under the
    // Bottom anchor (whose geometry the earlier waves
    // covered) paints its one cluster at the bottom
    // helper centre, disjoint from and far below the
    // top cluster — the two anchors are mirror
    // placements of the same layout, not one band
    // painted twice.
    let bottom = staged_project(1, vec![cue(0, 30, "HELLO")], SubtitleStyle::default());
    let img_b = render_frame(&bottom, 0);
    let clusters_b = row_clusters(&rows_with_face(&img_b));
    assert_eq!(clusters_b.len(), 1, "the bottom control paints one cluster");
    assert_cluster_at_helper_line(
        clusters_b[0],
        SubtitleStyle::default(),
        400.0,
        0,
        "bottom control cue",
    );
    assert!(
        clusters[0].1 < clusters_b[0].0,
        "the top cluster {:?} and the bottom cluster {:?} are disjoint, top first",
        clusters[0],
        clusters_b[0]
    );
}

#[test]
fn v10_top_anchor_stack_and_multiline_read_away_from_the_edge() {
    let style = top_style();

    // Two cues live at once: the FIRST cue sits
    // closest to the top edge and the second one line
    // further in — subtitle_stack_line_offset at the
    // top anchor counts away from the edge in cue
    // order (at Bottom the same pair stacks in the
    // same cue order from the bottom edge).
    let project = staged_project(
        1,
        vec![cue(0, 30, "HELLO"), cue(0, 30, "WORLD")],
        style,
    );
    let img = render_frame(&project, 0);
    let clusters = row_clusters(&rows_with_face(&img));
    assert_eq!(
        clusters.len(),
        2,
        "two stacked top cues paint two face clusters: {clusters:?}"
    );
    assert_cluster_at_helper_line(clusters[0], style, 400.0, 0, "first stacked top cue");
    assert_cluster_at_helper_line(clusters[1], style, 400.0, 1, "second stacked top cue");
    assert!(
        clusters[0].1 < clusters[1].0,
        "the stacked faces are disjoint, edge cue first: {clusters:?}"
    );

    // One cue, two lines: at the top anchor the cue's
    // lines read away from the edge in their own
    // order, so the narrow FIRST line ("I") sits in
    // the edge band and the wide second line one band
    // in — the exact inverse of the Bottom multiline
    // pin, where the last line takes the edge band.
    // Proven by face-pixel counts per band (the
    // in-crate pin's method): the bands are the
    // helper-derived partition [centre − lh/2,
    // centre + lh/2 + 3], which contains each line's
    // whole face ([centre − 8, centre + 11]) and
    // excludes both neighbours' faces.
    let band = |from_edge: usize| {
        let centre = subtitle_line_center_y(style, 400.0, from_edge);
        let half = subtitle_line_height(style) / 2.0;
        (centre - half).round() as u32..=(centre + half + 3.0).round() as u32
    };
    let multiline = staged_project(1, vec![cue(0, 30, "I\nWWWWWWWWWW")], style);
    let img_m = render_frame(&multiline, 0);
    let narrow = count_face_in_rows(&img_m, band(0));
    let wide = count_face_in_rows(&img_m, band(1));
    assert!(narrow > 0, "the cue's first line paints at the edge band");
    assert!(
        wide > 4 * narrow,
        "at Top the wide SECOND line must dominate the band one step in: \
         edge {narrow}, inner {wide}"
    );
}

#[test]
fn v10_top_anchor_half_open_liveness() {
    let style = top_style();
    // The cue is live on authored frames [2, 4) of
    // five (half-open, the house rule).
    let project = staged_project(5, vec![cue(2, 4, "HELLO")], style);
    let mut bare = project.clone();
    bare.subtitles.clear();

    for frame in [0usize, 1, 4] {
        let img = render_frame(&project, frame);
        assert_eq!(
            count_face(&img),
            0,
            "frame {frame} is outside the cue's half-open range and must carry no burn-in"
        );
    }
    // Past the range the render is the pre-subtitle
    // render, byte for byte (the Bottom anchor's
    // no-live-cue claim, now at Top).
    assert_eq!(
        render_frame(&project, 4).as_raw(),
        render_frame(&bare, 4).as_raw(),
        "a frame with no live cue renders exactly as the cueless project"
    );

    for frame in [2usize, 3] {
        let img = render_frame(&project, frame);
        let clusters = row_clusters(&rows_with_face(&img));
        assert_eq!(
            clusters.len(),
            1,
            "frame {frame} is inside the range and carries the top cue: {clusters:?}"
        );
        assert_cluster_at_helper_line(clusters[0], style, 400.0, 0, "live top cue");
    }
}

// ---------------------------------------------------------------------
// Leg 2 — JPEG import → sprite instance → scaled
// export, decoded and measured
// ---------------------------------------------------------------------

const JPEG_RGB: [u8; 3] = [200, 30, 30];

/// Per-channel tolerance for JPEG-decoded colour
/// (S10's): JPEG is lossy; the PNG sequence that
/// transports the render is not, so this is the only
/// tolerance in the leg.
const JPEG_TOL: u8 = 16;

/// Solid-colour JPEG bytes, encoded with the image
/// crate exactly as the media module's own fixtures
/// build them (encoder-made, never hand-rolled).
fn jpeg_bytes(w: u32, h: u32, px: image::Rgb<u8>) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .expect("encode JPEG fixture");
    buf
}

/// A core on a bare 200x150 stage (background
/// 0x101010, nothing else that can paint), two empty
/// frames, at 24 fps (S10's bare-core shape).
fn bare_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core.project.stage = [200.0, 150.0];
    core.project.background = 0x101010;
    core.project.figures.clear();
    core.project.frames = (0..2)
        .map(|_| Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    core.project.sprites.clear();
    core.project.texts.clear();
    core.project.strokes.clear();
    core.project.subtitles.clear();
    core.project.assets.clear();
    core
}

/// Pixels within `tol` of `target` on every channel.
fn count_near(img: &Rgb8, target: [u8; 3], tol: u8) -> usize {
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

/// The bounding box (min x, min y, max x, max y) of
/// the pixels within `tol` of `target`. Only called
/// when such pixels are known to exist.
fn near_bbox(img: &Rgb8, target: [u8; 3], tol: u8) -> (u32, u32, u32, u32) {
    let (mut x0, mut y0) = (u32::MAX, u32::MAX);
    let (mut x1, mut y1) = (0u32, 0u32);
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y).0;
            if p.iter().zip(target).all(|(a, b)| a.abs_diff(b) <= tol) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0, y0, x1, y1)
}

/// Decode one exported PNG frame.
fn decode_png(path: &Path) -> Rgb8 {
    image::open(path)
        .unwrap_or_else(|e| panic!("PNG {} decodes: {e}", path.display()))
        .to_rgb8()
}

/// Assert a decoded frame's JPEG-colour quad: centre
/// pixel in tolerance, bounding box of the expected
/// size about the expected centre, coverage near the
/// quad's full area (edge softening is the only
/// honest exception). Returns the coverage count for
/// the caller's cross-scale ratio.
fn assert_jpeg_quad(
    img: &Rgb8,
    centre: (u32, u32),
    size: (u32, u32),
    what: &str,
) -> usize {
    let c = img.get_pixel(centre.0, centre.1).0;
    for (channel, want) in c.iter().zip(JPEG_RGB) {
        assert!(
            channel.abs_diff(want) <= JPEG_TOL,
            "{what}: centre pixel {c:?} must sit within ±{JPEG_TOL} of {JPEG_RGB:?} per channel"
        );
    }
    let (x0, y0, x1, y1) = near_bbox(img, JPEG_RGB, JPEG_TOL);
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    assert!(
        w.abs_diff(size.0) <= 3 && h.abs_diff(size.1) <= 3,
        "{what}: quad measures {w}x{h}, expected {}x{} (the asset size at this scale)",
        size.0,
        size.1
    );
    let (bcx, bcy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
    assert!(
        (bcx - centre.0 as f32).abs() <= 3.0 && (bcy - centre.1 as f32).abs() <= 3.0,
        "{what}: quad centre ({bcx}, {bcy}) must sit at the instance position {centre:?}"
    );
    let covered = count_near(img, JPEG_RGB, JPEG_TOL);
    let area = size.0 * size.1;
    assert!(
        covered >= area as usize * 3 / 4 && covered <= area as usize * 5 / 4,
        "{what}: {covered} px carry the JPEG colour, expected near the quad's {area} px area"
    );
    covered
}

#[test]
fn v10_jpeg_import_sprite_instance_scaled_export_chain() {
    let dir = temp_dir("jpeg-chain");
    let mut core = bare_core();

    // Hop 1: the import funnel, through the registry.
    // The file's name does not say JPEG — content
    // decides (R9's magic rule).
    let jpeg = jpeg_bytes(24, 18, image::Rgb(JPEG_RGB));
    assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]), "fixture is a real JPEG");
    let file = dir.join("photo.bin");
    std::fs::write(&file, &jpeg).expect("write import file");
    let imported = invoke(
        &mut core,
        "asset_import",
        &json!({ "path": file.to_string_lossy() }),
    )
    .expect("the JPEG imports through the registry funnel");
    let asset = imported["index"].as_u64().expect("import returns an index") as usize;
    assert_eq!(imported["width"].as_u64(), Some(24));
    assert_eq!(imported["height"].as_u64(), Some(18));
    assert_eq!(
        core.project.assets[asset].png, jpeg,
        "the funnel stored the JPEG's bytes verbatim"
    );

    // Hop 2: the sprite instance, through the
    // registry. `sprite_add` places the instance at
    // the stage centre (100, 75 on this stage) at
    // frame 0, scale 1.0 — the quad is the asset's
    // native 24x18 about that centre.
    let added = invoke(&mut core, "sprite_add", &json!({ "asset": asset }))
        .expect("the imported asset instances as a sprite");
    let sprite = added["index"].as_u64().expect("sprite_add returns an index") as usize;
    assert_eq!(core.project.sprites.len(), 1);
    assert_eq!(core.project.sprites[sprite].asset, asset);
    let key = core.project.sprites[sprite].keys[0];
    assert_eq!(
        (key.x, key.y, key.scale),
        (100.0, 75.0, 1.0),
        "the instance key is the stage centre at scale 1.0"
    );

    // Hop 3: the scaled sequence export. At 2.0 the
    // frame is 400x300 and the instance's quad doubles
    // with the picture: 48x36 about (200, 150).
    let scaled_dir = dir.join("scaled");
    let n = export_png_sequence_scaled(&core.project, &scaled_dir, 2.0)
        .expect("scaled PNG sequence exports");
    assert_eq!(n, 2, "both authored frames export");
    let first = decode_png(&scaled_dir.join("frame-0001.png"));
    assert_eq!((first.width(), first.height()), (400, 300));
    let covered2 = assert_jpeg_quad(&first, (200, 150), (48, 36), "scaled frame 1");
    // The instance's single key holds across the
    // sequence (InstanceKey::sample's hold rule): the
    // second frame carries the quad too.
    let second = decode_png(&scaled_dir.join("frame-0002.png"));
    assert!(
        count_near(&second, JPEG_RGB, JPEG_TOL) >= 1300,
        "frame 2 of the scaled sequence still carries the sprite's quad"
    );

    // The unscaled control: the same chain at 1.0
    // measures the native quad, and the coverage
    // ratio between the two scales is the area ratio
    // — the relational half of the scale claim.
    let plain_dir = dir.join("plain");
    assert_eq!(
        export_png_sequence(&core.project, &plain_dir).expect("control sequence exports"),
        2
    );
    let control = decode_png(&plain_dir.join("frame-0001.png"));
    assert_eq!((control.width(), control.height()), (200, 150));
    let covered1 = assert_jpeg_quad(&control, (100, 75), (24, 18), "control frame 1");
    let ratio = covered2 as f32 / covered1 as f32;
    assert!(
        (3.2..=4.8).contains(&ratio),
        "scaled/control quad coverage ratio is {ratio}, expected ~4.0 (area doubles per axis)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — figure-library name fidelity (against
// V1's board contract; see the module docs)
// ---------------------------------------------------------------------

/// Serialises the leg-3 tests: they each change the
/// process current directory, which is process-global
/// (the wave_p_figlib discipline). Poison-tolerant.
static FIGLIB_CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
    FIGLIB_CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores the process current directory on drop,
/// including when the test panics.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> Self {
        let old = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(dir).expect("enter temp dir");
        CwdGuard(old)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// The library's entry names as `figure_library_list`
/// reports them, in slot order.
fn library_names(core: &mut EditorCore) -> Vec<String> {
    let payload = invoke(core, "figure_library_list", &json!({}))
        .expect("figure_library_list answers");
    payload["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("figure_library_list carries no entries array: {payload}"))
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("a library entry carries its name")
                .to_string()
        })
        .collect()
}

#[test]
fn v10_figure_library_name_keeps_its_case_end_to_end() {
    let _lock = lock_cwd();
    let dir = temp_dir("figlib-case");
    let _cwd = CwdGuard::enter(&dir);
    let mut core = EditorCore::new();

    // Save a figure named "Styk" through the registry
    // (the wave_p_figlib seeding recipe).
    let added = invoke(
        &mut core,
        "add_figure",
        &json!({ "kind": "mini", "name": "Styk" }),
    )
    .expect("add_figure answers");
    let figure = added["figure"]
        .as_u64()
        .unwrap_or_else(|| panic!("add_figure reply carries no figure index: {added}"));
    invoke(&mut core, "figure_library_save", &json!({ "figure": figure }))
        .expect("figure_library_save answers");

    // V1's contract, first half: the listing shows
    // the name EXACTLY as saved — "Styk", not the
    // lower-cased file stem the pre-V1 tree produced.
    let names = library_names(&mut core);
    assert!(
        names.iter().any(|n| n == "Styk"),
        "the saved figure must be listed as 'Styk', got: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "styk"),
        "no entry may carry the case-lost name 'styk', got: {names:?}"
    );

    // Second half: inserting BY that name yields a
    // project figure named "Styk" — the name a user
    // saved is the name they get back.
    let inserted = invoke(
        &mut core,
        "figure_library_insert",
        &json!({ "name": "Styk" }),
    )
    .expect("insert by the exact saved name succeeds");
    assert_eq!(
        inserted["name"],
        json!("Styk"),
        "the insert reply names the inserted figure"
    );
    let idx = inserted["figure"]
        .as_u64()
        .expect("the insert reply carries the figure index") as usize;
    assert_eq!(
        core.project.figures[idx].name, "Styk",
        "the inserted figure itself carries the saved name, case intact"
    );

    drop(_cwd);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v10_figure_library_lowercase_files_from_before_still_load() {
    let _lock = lock_cwd();
    let dir = temp_dir("figlib-legacy");
    // A library written by the pre-V1 tree: the entry
    // lives in a lower-case file holding a bare
    // FigureDef document (the format
    // `save_figure_to_library` has always written).
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).expect("create the library folder");
    std::fs::write(
        lib.join("styk.json"),
        serde_json::to_string_pretty(&FigureDef::simple()).expect("serialise legacy entry"),
    )
    .expect("plant the legacy entry");
    let _cwd = CwdGuard::enter(&dir);

    // The core loads ./library at construction: the
    // legacy file must still list — under its stem
    // name, the only name the old format ever had —
    // and still insert under that name. V1's fix must
    // not orphan libraries already on disk.
    let mut core = EditorCore::new();
    let names = library_names(&mut core);
    assert!(
        names.iter().any(|n| n == "styk"),
        "the legacy lower-case entry must still be listed, got: {names:?}"
    );
    let inserted = invoke(
        &mut core,
        "figure_library_insert",
        &json!({ "name": "styk" }),
    )
    .expect("insert of the legacy entry succeeds");
    assert_eq!(
        inserted["name"],
        json!("styk"),
        "the legacy entry inserts under the name it has always carried"
    );

    drop(_cwd);
    let _ = std::fs::remove_dir_all(&dir);
}
