//! Wave U (U14) — raster determinism on a POPULATED
//! project: A-B-A byte identity through `export::
//! render_frame` and `export::render_frame_scaled` over
//! the full `Project::new_demo`, populated.
//!
//! What the earlier suites pinned, and what this one
//! adds:
//!
//! - P47 (`tests/export_determinism.rs`) pinned raster
//!   REPEAT identity; S6 (`tests/wave_s_determinism.rs`)
//!   carried the raster funnel to A-B-A — but on the
//!   demo cut down to 3 frames with each content class
//!   (plain, subtitled, credited, segment-text alpha,
//!   scaled) isolated in its own fixture. T9 pinned the
//!   combined paths on the SVG funnel over a BARE stage;
//!   T19 decoded scaled GIF/APNG/sheet output on a bare
//!   stage.
//! - No suite had put the whole population — the demo
//!   figure WITH authored bend data, weather, a credited
//!   multi-line cue and a media sprite — through the
//!   raster funnel at once and demanded A-B-A identity.
//!   That is this file: two A-B-A legs (1.0 and 2.0) on
//!   one populated fixture, plus the funnel's documented
//!   1.0 equivalence between the two public entries.
//!
//! Why the populated render is deterministic BY
//! CONSTRUCTION (the legs are the proof, this is the
//! audit trail — every claim read at the source):
//! - The funnel (`render_frame_at_scaled_inner` in
//!   src/export.rs) collects drawables into Vecs in
//!   stored order (figures, then segments/heads/rings/
//!   polys, then the weather block appending to the
//!   particle Vec) and the pixel pass is one sequential
//!   nested loop over them. No rayon/`par_iter`, no
//!   thread pool, anywhere in the path.
//! - No `HashMap`/`BTreeMap`/`HashSet` exists in
//!   src/export.rs, src/media.rs, src/subtitles.rs,
//!   src/vfx.rs or src/model.rs (grep-verified), so no
//!   hash iteration order can reach a pixel.
//! - Weather positions come from `vfx::weather_
//!   particles`, a pure function of (weather, frame,
//!   stage, fps) whose placement derives from `prand`,
//!   a wrapping integer hash (src/vfx.rs) — no RNG
//!   state, no wall clock.
//! - Subtitles: `live_subtitle_cues` walks the stored
//!   cue Vec in stored order under the half-open
//!   liveness rule, and the burn-in composes display
//!   lines via `subtitle_display_lines` (pure string
//!   work) at positions from the shared layout helpers
//!   (pure arithmetic).
//! - Media: `media_items` walks figures, then sprites,
//!   then texts, each in stored order; sprite keys
//!   sample by clamped interpolation (pure). Assets are
//!   decoded on each call — media.rs's own module doc
//!   states there is NO decode cache — so no cached
//!   decode can leak state between renders.
//! - The one process-global in src/export.rs,
//!   `ENCODE_SEQ`, only names ffmpeg temp files in
//!   `unique_temp`; the still-frame funnel never reads
//!   it. No `SystemTime`/`Instant`/`rand` appears in
//!   any file on the path (grep-verified).
//! - Poses and bends evaluate from stored data only:
//!   `poses_at` (tween playback is off in the demo, so
//!   the authored frame is taken verbatim) and
//!   `bends_at` (stored per-node radians, resized with
//!   zeros) — both pure functions of the project.
//!
//! Failure messages report the first differing byte
//! index and the differing-byte count rather than
//! dumping whole frames (the P47/S6 discipline), so a
//! CI failure carries the measured difference with it.
//!
//! Everything uses only the public API plus the
//! `image` crate's encoder for the fixture's PNG bytes
//! (the S10 idiom — encoder-made fixtures only). The
//! fixture sets fps 24.0 explicitly (board contract);
//! the funnel evaluates nothing by wall-clock time.

use std::io::Cursor;

use stykanimate_rs::export;
use stykanimate_rs::model::{ImageAsset, InstanceKey, Project, SpriteInstance};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};
use stykanimate_rs::vfx::{Weather, WeatherKind};

/// The frame every leg renders: the demo's most
/// displaced wave pose (root pushed +28, arm highest),
/// mid-timeline rather than the rest frame.
const FRAME: usize = 2;

/// The per-node bend (radians) the fixture authors onto
/// every non-root node of the demo figure at FRAME, and
/// the deepened value leg 1's B render uses instead.
const BEND_A: f32 = 0.35;
const BEND_B: f32 = 0.7;

// ---------------------------------------------------------------------
// Fixture — the full demo project, populated
// ---------------------------------------------------------------------

/// Solid-colour PNG bytes, encoded with the image
/// crate (the S10/T9 idiom — encoder-made fixtures
/// only; hand-rolled PNG bytes are what broke CI
/// round 4 of the Wave R line).
fn png_bytes(w: u32, h: u32, px: image::Rgba<u8>) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    buf
}

/// Author bend data onto `frame`: every non-root node
/// of the (single) demo figure bent by `bend` radians.
/// The demo ships `bends: Vec::new()` on every frame,
/// so a populated project must write the data itself —
/// `bends_at` reads `frames[f].bends[figure][node]`,
/// resizing with zeros, so one inner Vec per figure is
/// the whole contract.
fn author_bends(project: &mut Project, frame: usize, bend: f32) {
    let n = project.figures[0].def.nodes.len();
    assert!(n > 2, "the demo figure must have nodes to bend");
    let mut bends = vec![0.0f32; n];
    for b in bends.iter_mut().skip(1) {
        *b = bend;
    }
    project.frames[frame].bends = vec![bends];
}

/// The populated project: `Project::new_demo` intact
/// (one humanoid figure, five authored frames, 960x720
/// stage — nothing cleared, nothing truncated), at an
/// explicit 24 fps, PLUS the four population classes:
///
/// - authored per-node bends on the rendered frame
///   (the demo's own frames carry none);
/// - rain weather (40 drops, wind 30 — T9's rain);
/// - one credited TWO-LINE subtitle cue live across
///   the whole timeline (half-open [0, 5)), with an
///   explicit subtitle style (the S6 idiom: bottom,
///   scale 1.0, white on black — set, not inherited);
/// - one media sprite: an encoder-made 48x36 PNG in
///   the asset table, instanced at scale 2 (a 96x72
///   quad) centred at (200, 200) — clear of the figure
///   at centre stage. Its single key sits at frame
///   0.0; `InstanceKey::sample` clamps, so the key
///   holds at frame 2.
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
// Helpers
// ---------------------------------------------------------------------

/// Byte-identity with a compact measured difference on
/// failure: first differing index + how many bytes
/// differ + both lengths (S6's helper, verbatim
/// discipline).
fn assert_bytes_identical(a: &[u8], b: &[u8], context: &str) {
    if a == b {
        return;
    }
    let shared = a.len().min(b.len());
    let first_diff = (0..shared).find(|&i| a[i] != b[i]);
    let differing = (0..shared).filter(|&i| a[i] != b[i]).count() + a.len().abs_diff(b.len());
    panic!(
        "{context}: frames differ — first differing byte at {first_diff:?}, \
         {differing} bytes differ (lengths {} vs {})",
        a.len(),
        b.len()
    );
}

// ---------------------------------------------------------------------
// Leg 1 — the populated frame through render_frame (1.0)
// ---------------------------------------------------------------------

#[test]
fn raster_aba_populated_project_frame_is_byte_identical() {
    let base = populated_fixture();

    let a1 = export::render_frame(&base, FRAME);
    assert_eq!(
        (a1.width(), a1.height()),
        (960, 720),
        "the 1.0 render keeps the demo stage size"
    );

    // Immediate repeat: byte-identical.
    let a_repeat = export::render_frame(&base, FRAME);
    assert_bytes_identical(
        a1.as_raw(),
        a_repeat.as_raw(),
        "populated frame rendered twice back to back",
    );

    // Non-vacuity witnesses — one per population class.
    // Each renders an otherwise-identical project with
    // exactly one class removed; each must move the
    // raster, or that class is not reaching the
    // rasterizer and the A-B-A demand below would be
    // vacuous for it. They double as interleaves: four
    // unrelated renders sit between the repeat and B.
    let mut no_bends = base.clone();
    no_bends.frames[FRAME].bends.clear();
    assert_ne!(
        a1.as_raw(),
        export::render_frame(&no_bends, FRAME).as_raw(),
        "clearing the authored bends changed no raster output — \
         bend data is not reaching the rasterizer"
    );

    let mut no_weather = base.clone();
    no_weather.weather = None;
    assert_ne!(
        a1.as_raw(),
        export::render_frame(&no_weather, FRAME).as_raw(),
        "removing the rain changed no raster output — \
         the weather is not reaching the rasterizer"
    );

    let mut no_sprite = base.clone();
    no_sprite.sprites.clear();
    assert_ne!(
        a1.as_raw(),
        export::render_frame(&no_sprite, FRAME).as_raw(),
        "removing the sprite changed no raster output — \
         the media composite is not reaching the rasterizer"
    );

    let mut uncredited = base.clone();
    uncredited.subtitles[0].speaker = None;
    assert_ne!(
        a1.as_raw(),
        export::render_frame(&uncredited, FRAME).as_raw(),
        "uncrediting the cue changed no raster output — \
         the speaker prefix is not reaching the burn-in"
    );

    // B: the bends deepened (0.35 -> 0.7 rad on every
    // non-root node) — the populated geometry path is
    // the mutation under interleave.
    let mut bent = base.clone();
    author_bends(&mut bent, FRAME, BEND_B);
    let b = export::render_frame(&bent, FRAME);
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "deepening the bends changed no raster output — \
         the changed project is not reaching the rasterizer"
    );

    // Back to A: byte-identical, no residue from the
    // witnesses or B.
    let a2 = export::render_frame(&base, FRAME);
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A populated frame: frame 2 before vs after \
         the witnesses and the deepened-bend render",
    );
}

// ---------------------------------------------------------------------
// Leg 2 — the populated frame through render_frame_scaled (2.0)
// ---------------------------------------------------------------------

#[test]
fn raster_aba_populated_project_scaled_frame_is_byte_identical() {
    let base = populated_fixture();

    let a1 = export::render_frame_scaled(&base, FRAME, 2.0)
        .expect("the 2.0 A render is not refused");
    assert_eq!(
        (a1.width(), a1.height()),
        (1920, 1440),
        "the 2.0 render doubles the demo stage size"
    );

    // Immediate repeat at scale: byte-identical.
    let a_repeat = export::render_frame_scaled(&base, FRAME, 2.0)
        .expect("the 2.0 repeat render is not refused");
    assert_bytes_identical(
        a1.as_raw(),
        a_repeat.as_raw(),
        "populated frame at 2.0 rendered twice back to back",
    );

    // B: the speaker renamed (MARA -> JUNIPER). Only the
    // composed prefix can move — B proves the interleave
    // exercises the scaled burn-in (the IA-R10-3 path:
    // the burn-in's layout multiplied by the same
    // scale), not just generic geometry.
    let mut renamed = base.clone();
    renamed.subtitles[0].speaker = Some("JUNIPER".to_string());
    let b = export::render_frame_scaled(&renamed, FRAME, 2.0)
        .expect("the 2.0 B render is not refused");
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "renaming the speaker changed no raster output at 2.0 — \
         the scaled burn-in is not reaching the rasterizer"
    );

    // Back to A: byte-identical, no residue.
    let a2 = export::render_frame_scaled(&base, FRAME, 2.0)
        .expect("the second 2.0 A render is not refused");
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A populated frame at 2.0: frame 2 before vs \
         after the renamed-speaker render",
    );
}

// ---------------------------------------------------------------------
// Leg 3 — the funnel's documented 1.0 equivalence, populated
// ---------------------------------------------------------------------

/// `render_frame_scaled`'s own doc promises its 1.0
/// result is byte-identical to `render_frame`. S6
/// pinned the scaled funnel through
/// `render_frame_at_scaled`; this leg pins the two
/// PUBLIC entries against each other on the populated
/// fixture, so the equivalence is proven where the
/// population (bends + weather + credit + sprite) can
/// break it, not only on S6's isolated fixtures.
#[test]
fn render_frame_scaled_at_1_0_matches_render_frame_on_the_populated_project() {
    let base = populated_fixture();

    let plain = export::render_frame(&base, FRAME);
    let scaled = export::render_frame_scaled(&base, FRAME, 1.0)
        .expect("the 1.0 scaled render is not refused");
    assert_eq!(
        (plain.width(), plain.height()),
        (scaled.width(), scaled.height()),
        "the two entries disagree on the 1.0 output size"
    );
    assert_bytes_identical(
        plain.as_raw(),
        scaled.as_raw(),
        "render_frame vs render_frame_scaled at 1.0 on the populated project",
    );
}
