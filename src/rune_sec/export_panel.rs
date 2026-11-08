//! Export panel for the Rune editor shell (Phase D,
//! section 20/40). Declared from `main.rs` only — bin-crate
//! code, not part of the engine library.
//!
//! The Rune successor of the GPUI export centre: the same 15
//! formats under the same `xc-*` element keys, grouped in the
//! Wave 15 categories (Images / Animation / Video / Project
//! formats), each row a button plus the GPUI centre's guidance
//! note (paths verified against the ops in `editor.rs`).
//!
//! Every button calls a real export. In "All frames" mode the
//! handlers are exactly the `EditorCore` ops the GPUI centre
//! called. Six formats also have frame-range exporters in
//! `export.rs` (PNG/JPEG/GIF/APNG/MP4/WebM — the same free
//! functions the MCP layer calls, e.g. `mcp_io.rs`'s
//! `export_mp4_range`); when the panel's range toggle is set
//! to "Loop range" and a loop range exists, those six write
//! only the loop-range frames, to the same destinations the
//! full ops use, and report through `core.status` in the
//! full ops' wording. SVG, the spritesheet and all project
//! formats have no range variant, so they always export in
//! full — the toggle line says so rather than pretending
//! otherwise. MP4/WebM range exports are video-only (the
//! full MP4 export muxes audio; the range path cannot), and
//! the notes say that too.
//!
//! Two bundle buttons sit under the categories: "Export
//! all images" (PNG + JPEG + SVG + spritesheet) and "Export
//! all video" (MP4 + WebM). A bundle is a composition of the
//! same full `EditorCore` ops the single-format buttons call
//! in "All frames" mode, run in `plan_bundle` order. Bundles
//! deliberately ignore the range toggle: of the bundle
//! members only PNG/JPEG/MP4/WebM have range exporters at
//! all, and a bundle that silently ranged some formats and
//! not others would not match its label — the bundle rows'
//! notes say "always export in full" instead. Each op sets
//! `core.status` as it runs; when the bundle finishes, the
//! handler overwrites the status with one recap line (built
//! by `bundle_recap` from the ops actually called, naming
//! any that failed) and stores the same line in the panel
//! recap.
//!
//! The recap line under the bundles shows the last export —
//! bundle or single — as "Exported <name> → <destination>",
//! or "<name> export failed" when the op reported failure.
//! Failure detection reads `core.status`: the export ops
//! return `()` and report only through the status line, and
//! every failure wording in `editor.rs` contains "failed"
//! (see `export_failed`).
//!
//! Panel state: the toggle position lives in
//! `State::export_use_range` and the recap line in
//! `State::export_recap` (see the integration report —
//! `main.rs` owns `State`; the integrator adds
//! `pub export_recap: String`, initialised to
//! `String::new()`). The range itself is the core's real
//! `loop_range`, set in the timeline; this panel only
//! reads it.

use std::path::PathBuf;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::State;

/// The `xc-*` keys whose formats have a frame-range exporter
/// in `export.rs`. Everything else always exports in full.
pub const RANGE_KEYS: [&str; 6] = ["xc-png", "xc-jpeg", "xc-gif", "xc-apng", "xc-mp4", "xc-webm"];

/// Does the format behind this `xc-*` key support frame-range
/// export? (PNG/JPEG sequences, GIF, APNG, MP4, WebM — the
/// `*_range` functions in `export.rs`.)
pub fn supports_range(key: &str) -> bool {
    RANGE_KEYS.contains(&key)
}

/// The frame range a range-capable export should write, if
/// any: `Some((from, to))` (inclusive, ordered, clamped to
/// the timeline) when the toggle selects the loop range and
/// a loop range is set; `None` means "export everything",
/// which the caller does by invoking the full `EditorCore`
/// op. The core's `set_loop_range` already orders and
/// clamps its endpoints, but frames can be deleted after a
/// range is set, so the clamp is applied again here.
pub fn range_target(core: &EditorCore, use_range: bool) -> Option<(usize, usize)> {
    if !use_range {
        return None;
    }
    let (a, b) = core.loop_range?;
    let n = core.project.frames.len();
    if n == 0 {
        return None;
    }
    let a = a.min(n - 1);
    let b = b.min(n - 1);
    Some((a.min(b), a.max(b)))
}

/// The muted note shown when a loop range is set — the same
/// facts the GPUI centre printed (frames are 1-based in the
/// UI), updated for this panel: here the range exports are
/// one toggle away, and video range exports are silent.
pub fn loop_note(core: &EditorCore) -> Option<String> {
    let (a, b) = range_target(core, true)?;
    Some(format!(
        "Loop range: frames {}–{} — with \"Loop range\" selected below, the PNG/JPEG/GIF/APNG/MP4/WebM exports write only those frames (MP4/WebM range exports are video-only).",
        a + 1,
        b + 1
    ))
}

/// The line under the range toggle stating what the next
/// export will actually cover, in every toggle/range
/// combination — including the honest fallback when "Loop
/// range" is selected but no loop range exists.
pub fn effective_text(core: &EditorCore, use_range: bool) -> String {
    let n = core.project.frames.len();
    if n == 0 {
        return "The project has no frames to export.".into();
    }
    match (use_range, range_target(core, true)) {
        (true, Some((a, b))) => format!(
            "Range-capable exports write frames {}–{} (the loop range); other formats export in full.",
            a + 1,
            b + 1
        ),
        (true, None) => {
            "Loop range selected, but no loop range is set — exports write all frames. Set a loop range in the timeline first.".into()
        }
        (false, _) => format!("Exports write all {n} frames."),
    }
}

/// The `./export` directory under the working directory —
/// the destination the `EditorCore` media ops build their
/// paths from (`editor.rs` `export_gif`/`export_png`/...),
/// mirrored here for the range calls.
fn export_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("export")
}

// ---- Bundle plans and recaps -----------------------------
// `plan_bundle` is the single source of truth for what a
// bundle runs: the bundle handler executes exactly this
// list, in this order, and `bundle_recap` reports exactly
// this list. Keeping both on the plan means the recap can
// never claim a format the bundle did not call.

/// One media export operation a bundle (or a single-format
/// button) can run. Each variant maps to exactly one
/// `EditorCore` op — the same call the format's own button
/// makes in "All frames" mode (see `run_export_op`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportOp {
    Png,
    Jpeg,
    Svg,
    Spritesheet,
    Gif,
    Apng,
    Mp4,
    Webm,
}

impl ExportOp {
    /// The format's display name, as used in recaps.
    pub fn name(self) -> &'static str {
        match self {
            ExportOp::Png => "PNG",
            ExportOp::Jpeg => "JPEG",
            ExportOp::Svg => "SVG",
            ExportOp::Spritesheet => "sheet",
            ExportOp::Gif => "GIF",
            ExportOp::Apng => "APNG",
            ExportOp::Mp4 => "MP4",
            ExportOp::Webm => "WebM",
        }
    }

    /// Where the format's full export writes — the same
    /// destinations the rows' guidance notes print (JPEG and
    /// APNG are the odd ones out: `out/`, not `./export/`).
    pub fn dest(self) -> &'static str {
        match self {
            ExportOp::Png => "./export/",
            ExportOp::Jpeg => "out/jpeg/",
            ExportOp::Svg => "./export/svg/",
            ExportOp::Spritesheet => "./export/spritesheet.png",
            ExportOp::Gif => "./export/animation.gif",
            ExportOp::Apng => "out/animation.apng",
            ExportOp::Mp4 => "./export/animation.mp4",
            ExportOp::Webm => "./export/animation.webm",
        }
    }
}

/// Which bundle a plan (or a bundle button) describes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BundleKind {
    /// "Export all images": PNG + JPEG + SVG + spritesheet.
    Images,
    /// "Export all video": MP4 + WebM.
    Video,
}

/// The ops a bundle runs, in execution order. Pure, so the
/// order and membership are unit-testable without exporting
/// anything.
pub fn plan_bundle(kind: BundleKind) -> Vec<ExportOp> {
    match kind {
        BundleKind::Images => vec![
            ExportOp::Png,
            ExportOp::Jpeg,
            ExportOp::Svg,
            ExportOp::Spritesheet,
        ],
        BundleKind::Video => vec![ExportOp::Mp4, ExportOp::Webm],
    }
}

/// Did the export op that just ran report failure? The
/// `EditorCore` export ops return `()` and report only
/// through `core.status`; every failure wording in
/// `editor.rs` contains "failed" ("Export failed: …",
/// "JPEG export failed: …", "Spritesheet failed: …", …)
/// and no success wording does, so the status line is the
/// failure signal. Checked immediately after each op call,
/// before anything else can overwrite the status.
pub fn export_failed(core: &EditorCore) -> bool {
    core.status.to_lowercase().contains("failed")
}

/// The recap line for one finished single-format export:
/// name + destination ("Exported PNG → ./export/").
pub fn single_recap(name: &str, dest: &str) -> String {
    format!("Exported {name} → {dest}")
}

/// The recap line for a single-format export whose op
/// reported failure — the recap never claims a failed
/// export as exported.
pub fn failed_recap(name: &str) -> String {
    format!("{name} export failed")
}

/// Record the outcome of a single-format export in the
/// panel's recap line: `single_recap` on success,
/// `failed_recap` when the op reported failure.
fn note_single(s: &mut State, op: ExportOp) {
    s.export_recap = if export_failed(&s.core) {
        failed_recap(op.name())
    } else {
        single_recap(op.name(), op.dest())
    };
}

/// The recap line for a finished bundle, built from the
/// plan: every op that ran and did not fail, with its
/// destination, in plan order; the failed ops (if any) are
/// named after a "; failed:" marker instead of being
/// claimed as exported.
pub fn bundle_recap(kind: BundleKind, failed: &[ExportOp]) -> String {
    let mut done: Vec<String> = Vec::new();
    for op in plan_bundle(kind) {
        if !failed.contains(&op) {
            done.push(format!("{} → {}", op.name(), op.dest()));
        }
    }
    let mut recap = if done.is_empty() {
        "Exported nothing".to_string()
    } else {
        format!("Exported {}", done.join(", "))
    };
    if !failed.is_empty() {
        let names: Vec<&str> = failed.iter().map(|op| op.name()).collect();
        recap += &format!("; failed: {}", names.join(", "));
    }
    recap
}

/// Run one planned op — exactly the call the format's
/// single-format button makes in "All frames" mode — and
/// report whether the op signalled failure.
fn run_export_op(s: &mut State, op: ExportOp) -> bool {
    match op {
        ExportOp::Png => s.core.export_png(),
        ExportOp::Jpeg => s.core.export_jpeg_dir(),
        ExportOp::Svg => s.core.export_svg(),
        ExportOp::Spritesheet => s.core.export_sheet(),
        ExportOp::Gif => s.core.export_gif(),
        ExportOp::Apng => s.core.export_apng_file(),
        ExportOp::Mp4 => s.core.export_mp4(),
        ExportOp::Webm => s.core.export_webm(),
    }
    export_failed(&s.core)
}

/// Run a whole bundle: every op in `plan_bundle` order,
/// always the full ops (bundles ignore the range toggle —
/// see the module docs). Each op overwrites `core.status`
/// as it runs; when the bundle finishes, this handler has
/// the last word: it overwrites the status with the bundle
/// recap and stores the same line as the panel recap, so
/// both show the summary rather than whichever op ran last.
fn run_bundle(s: &mut State, kind: BundleKind) {
    let mut failed: Vec<ExportOp> = Vec::new();
    for op in plan_bundle(kind) {
        if run_export_op(s, op) {
            failed.push(op);
        }
    }
    let recap = bundle_recap(kind, &failed);
    s.core.status = recap.clone();
    s.export_recap = recap;
}

// ---- Range-capable actions --------------------------------
// Each falls back to the full `EditorCore` op when no range
// target applies, so the button never silently does less
// than its label promises. Destinations and status wording
// mirror the full ops in `editor.rs`.

fn export_png_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            match stykanimate_rs::export::export_png_sequence_range(&s.core.project, a, b, &dir) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n} PNG frames (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        dir.display()
                    )
                }
                Err(e) => s.core.status = format!("Export failed: {e}"),
            }
        }
        None => s.core.export_png(),
    }
    note_single(s, ExportOp::Png);
}

fn export_jpeg_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = PathBuf::from("out").join("jpeg");
            match stykanimate_rs::export::export_jpeg_sequence_range(&s.core.project, a, b, &dir) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n} JPEG frames (frames {}–{}) to out/jpeg/.",
                        a + 1,
                        b + 1
                    )
                }
                Err(e) => s.core.status = format!("JPEG export failed: {e}"),
            }
        }
        None => s.core.export_jpeg_dir(),
    }
    note_single(s, ExportOp::Jpeg);
}

fn export_gif_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("animation.gif");
            match stykanimate_rs::export::export_gif_range(&s.core.project, a, b, &path) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n}-frame GIF (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        path.display()
                    )
                }
                Err(e) => s.core.status = format!("GIF export failed: {e}"),
            }
        }
        None => s.core.export_gif(),
    }
    note_single(s, ExportOp::Gif);
}

fn export_apng_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = PathBuf::from("out");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("animation.apng");
            match stykanimate_rs::export::export_apng_range(&s.core.project, a, b, &path) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n}-frame APNG (frames {}–{}) to out/animation.apng.",
                        a + 1,
                        b + 1
                    )
                }
                Err(e) => s.core.status = format!("APNG export failed: {e}"),
            }
        }
        None => s.core.export_apng_file(),
    }
    note_single(s, ExportOp::Apng);
}

fn export_mp4_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("animation.mp4");
            // Video only: unlike the full op, the range path
            // never muxes the project's audio (see
            // `export::export_mp4_range` docs).
            match stykanimate_rs::export::export_mp4_range(&s.core.project, a, b, &path) {
                Ok(()) => {
                    s.core.status = format!(
                        "Exported MP4 (frames {}–{}, video only) to {}",
                        a + 1,
                        b + 1,
                        path.display()
                    )
                }
                Err(e) => s.core.status = format!("MP4 export failed: {e}"),
            }
        }
        None => s.core.export_mp4(),
    }
    note_single(s, ExportOp::Mp4);
}

fn export_webm_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("animation.webm");
            match stykanimate_rs::export::export_webm_range(&s.core.project, a, b, &path) {
                Ok(()) => {
                    s.core.status = format!(
                        "Exported WebM (frames {}–{}, video only) to {}",
                        a + 1,
                        b + 1,
                                    path.display()
                    )
                }
                Err(e) => s.core.status = format!("WebM export failed: {e}"),
            }
        }
        None => s.core.export_webm(),
    }
    note_single(s, ExportOp::Webm);
}

// ---- Rows and sections ------------------------------------

/// A section column: the category label, then one row per
/// format (Wave 15 grouping: Images / Animation / Video /
/// Project formats).
fn section(title: &str, t: &Theme) -> El {
    col()
        .gap(6.0)
        .child(text(title).font(type_::SECTION).color(t.text_secondary))
}

/// One format row for a full-export-only format: an `xc-`
/// button calling the `EditorCore` op, plus its muted
/// guidance note — the GPUI `xc_row` shape. The handler
/// also records the panel recap (`recap_name` +
/// `recap_dest`, or a failure note when the op reports
/// one — see `note_single`).
fn plain_row(
    key: &'static str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    op: fn(&mut EditorCore),
    recap_name: &'static str,
    recap_dest: &'static str,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(btn(key, label, false, t, move |s: &mut State| {
            op(&mut s.core);
            s.export_recap = if export_failed(&s.core) {
                failed_recap(recap_name)
            } else {
                single_recap(recap_name, recap_dest)
            };
        }))
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// One format row for a range-capable format: the button's
/// handler is one of the `export_*_action` functions above,
/// which choose between the range exporter and the full op
/// at press time.
fn range_row(
    key: &'static str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    action: fn(&mut State),
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(btn(key, label, false, t, action))
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// One bundle row: an `xc-bundle-*` button running
/// `run_bundle` for its kind, plus the note stating that
/// bundles always export in full (the range toggle governs
/// only the single-format buttons).
fn bundle_row(
    key: &'static str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    kind: BundleKind,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(btn(key, label, false, t, move |s: &mut State| {
            run_bundle(s, kind)
        }))
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// The export panel: header, the loop-range note (when a
/// range is set), the All frames / Loop range toggle with
/// its effective-behaviour line, the four category sections
/// with all 15 `xc-*` rows, the Bundles section (two
/// `xc-bundle-*` rows), and — once anything has been
/// exported — the recap line showing the last export.
pub fn export_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let use_range = s.export_use_range;
    let mut panel = col().key("export-panel").w_full().gap(10.0);
    panel = panel.child(text("Export").font(type_::TITLE).color(t.text));
    panel = panel.child(
        text("Export centre — every format in one place")
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    if let Some(note) = loop_note(core) {
        panel = panel.child(text(note).font(type_::LABEL).color(t.text_secondary));
    }
    let toggle = row()
        .key("xc-range-toggle")
        .gap(8.0)
        .center_y()
        .child(text("Frames:").font(type_::LABEL).color(t.text))
        .child(btn("xc-range-all", "All frames", !use_range, t, |s| {
            s.export_use_range = false;
        }))
        .child(btn("xc-range-loop", "Loop range", use_range, t, |s| {
            s.export_use_range = true;
        }));
    panel = panel.child(toggle);
    panel = panel.child(
        text(effective_text(core, use_range))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    let images = section("Images", t)
        .child(range_row(
            "xc-png",
            "PNG",
            "PNG image sequence — one file per frame in ./export/",
            t,
            export_png_action,
        ))
        .child(range_row(
            "xc-jpeg",
            "JPEG",
            "JPEG image sequence in out/jpeg/ (out/, not export/)",
            t,
            export_jpeg_action,
        ))
        .child(plain_row(
            "xc-svg",
            "SVG",
            "One SVG per frame in ./export/svg/",
            t,
            EditorCore::export_svg,
            ExportOp::Svg.name(),
            ExportOp::Svg.dest(),
        ));
    panel = panel.child(images);

    let animation = section("Animation", t)
        .child(plain_row(
            "xc-sheet",
            "Spritesheet",
            "All frames on one sheet — ./export/spritesheet.png",
            t,
            EditorCore::export_sheet,
            ExportOp::Spritesheet.name(),
            ExportOp::Spritesheet.dest(),
        ))
        .child(range_row(
            "xc-gif",
            "GIF",
            "Animated GIF — ./export/animation.gif",
            t,
            export_gif_action,
        ))
        .child(range_row(
            "xc-apng",
            "APNG",
            "Animated PNG — out/animation.apng (out/, not export/)",
            t,
            export_apng_action,
        ));
    panel = panel.child(animation);

    let video = section("Video", t)
        .child(range_row(
            "xc-mp4",
            "MP4",
            "MP4 video via ffmpeg — ./export/animation.mp4 (full export includes audio; range export is video-only)",
            t,
            export_mp4_action,
        ))
        .child(range_row(
            "xc-webm",
            "WebM",
            "WebM video (VP9) — ./export/animation.webm",
            t,
            export_webm_action,
        ));
    panel = panel.child(video);

    let formats = section("Project formats", t)
        .child(plain_row(
            "xc-v2",
            ".piv v2",
            "Pivot v2 project — ./export/animation-v2.piv",
            t,
            EditorCore::export_piv2,
            ".piv v2",
            "./export/animation-v2.piv",
        ))
        .child(plain_row(
            "xc-v3",
            ".piv v3",
            "Pivot v3 project — ./export/animation.piv",
            t,
            EditorCore::export_piv,
            ".piv v3",
            "./export/animation.piv",
        ))
        .child(plain_row(
            "xc-v4",
            ".piv v4",
            "Pivot v4 project — ./export/animation-v4.piv",
            t,
            EditorCore::export_piv4,
            ".piv v4",
            "./export/animation-v4.piv",
        ))
        .child(plain_row(
            "xc-v5",
            ".piv v5",
            "Pivot v5 project — out/animation-v5.piv (out/, not export/)",
            t,
            EditorCore::export_v5_file,
            ".piv v5",
            "out/animation-v5.piv",
        ))
        .child(plain_row(
            "xc-stk",
            ".stk",
            "Selected figure only — ./export/<figure>.stk",
            t,
            EditorCore::export_stk,
            ".stk",
            "./export/<figure>.stk",
        ))
        .child(plain_row(
            "xc-nodes",
            ".nodes",
            "Selected figure for Stick Nodes — ./export/<figure>.nodes",
            t,
            EditorCore::export_nodes,
            ".nodes",
            "./export/<figure>.nodes",
        ))
        .child(plain_row(
            "xc-stknds",
            ".stknds",
            "Whole project for Stick Nodes — ./export/animation.stknds",
            t,
            EditorCore::export_stknds,
            ".stknds",
            "./export/animation.stknds",
        ));
    panel = panel.child(formats);

    let bundles = section("Bundles", t)
        .child(bundle_row(
            "xc-bundle-images",
            "Export all images",
            "PNG + JPEG + SVG + spritesheet in one press. Bundles always export in full — the range toggle above applies only to the single-format buttons.",
            t,
            BundleKind::Images,
        ))
        .child(bundle_row(
            "xc-bundle-video",
            "Export all video",
            "MP4 + WebM in one press, full exports (the MP4 keeps its audio). Bundles always export in full — the range toggle above applies only to the single-format buttons.",
            t,
            BundleKind::Video,
        ));
    panel = panel.child(bundles);

    if !s.export_recap.is_empty() {
        panel = panel.child(
            row().key("xc-recap").child(
                text(s.export_recap.clone())
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
        );
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_support_matches_the_export_rs_range_functions() {
        for key in ["xc-png", "xc-jpeg", "xc-gif", "xc-apng", "xc-mp4", "xc-webm"] {
            assert!(supports_range(key), "{key} should support range export");
        }
        for key in [
            "xc-svg", "xc-sheet", "xc-v2", "xc-v3", "xc-v4", "xc-v5", "xc-stk", "xc-nodes",
            "xc-stknds", "xc-bogus",
        ] {
            assert!(!supports_range(key), "{key} should not support range export");
        }
    }

    #[test]
    fn range_target_needs_the_toggle_and_a_loop_range() {
        let mut core = EditorCore::new();
        assert_eq!(range_target(&core, true), None, "no loop range set");
        core.loop_range = Some((1, 2));
        assert_eq!(range_target(&core, false), None, "toggle off");
        assert_eq!(range_target(&core, true), Some((1, 2)));
    }

    #[test]
    fn range_target_orders_and_clamps_to_the_timeline() {
        let mut core = EditorCore::new();
        let n = core.project.frames.len();
        assert!(n > 2, "demo project should have several frames");
        core.loop_range = Some((2, 1));
        assert_eq!(range_target(&core, true), Some((1, 2)), "inverted stored range");
        core.loop_range = Some((0, usize::MAX));
        assert_eq!(
            range_target(&core, true),
            Some((0, n - 1)),
            "range end past the timeline (frames deleted after setting it)"
        );
    }

    #[test]
    fn loop_note_names_the_range_one_based() {
        let mut core = EditorCore::new();
        assert_eq!(loop_note(&core), None);
        core.loop_range = Some((1, 2));
        let note = loop_note(&core).expect("note with a loop range set");
        assert!(note.contains("frames 2–3"), "unexpected note: {note}");
        assert!(note.contains("video-only"), "video caveat missing: {note}");
    }

    #[test]
    fn effective_text_covers_every_toggle_state() {
        let mut core = EditorCore::new();
        let n = core.project.frames.len();
        assert_eq!(effective_text(&core, false), format!("Exports write all {n} frames."));
        assert!(
            effective_text(&core, true).contains("no loop range is set"),
            "toggle on without a range must admit the fallback"
        );
        core.loop_range = Some((0, 1));
        assert!(effective_text(&core, true).contains("frames 1–2"));
        assert!(effective_text(&core, false).contains("all"));
    }

    #[test]
    fn images_bundle_plan_is_png_jpeg_svg_sheet_in_order() {
        assert_eq!(
            plan_bundle(BundleKind::Images),
            vec![
                ExportOp::Png,
                ExportOp::Jpeg,
                ExportOp::Svg,
                ExportOp::Spritesheet
            ]
        );
    }

    #[test]
    fn video_bundle_plan_is_mp4_then_webm() {
        assert_eq!(
            plan_bundle(BundleKind::Video),
            vec![ExportOp::Mp4, ExportOp::Webm]
        );
    }

    #[test]
    fn bundle_plans_have_no_duplicates_and_do_not_overlap() {
        for kind in [BundleKind::Images, BundleKind::Video] {
            let plan = plan_bundle(kind);
            let mut sorted = plan.clone();
            sorted.sort_by_key(|op| op.name());
            sorted.dedup();
            assert_eq!(sorted.len(), plan.len(), "{kind:?} plan repeats an op");
        }
        let images = plan_bundle(BundleKind::Images);
        for op in plan_bundle(BundleKind::Video) {
            assert!(!images.contains(&op), "{op:?} is in both bundles");
        }
    }

    #[test]
    fn bundle_recap_lists_every_planned_op_with_its_destination() {
        assert_eq!(
            bundle_recap(BundleKind::Images, &[]),
            "Exported PNG → ./export/, JPEG → out/jpeg/, SVG → ./export/svg/, sheet → ./export/spritesheet.png"
        );
        assert_eq!(
            bundle_recap(BundleKind::Video, &[]),
            "Exported MP4 → ./export/animation.mp4, WebM → ./export/animation.webm"
        );
    }

    #[test]
    fn bundle_recap_names_failures_instead_of_claiming_them() {
        assert_eq!(
            bundle_recap(BundleKind::Video, &[ExportOp::Mp4]),
            "Exported WebM → ./export/animation.webm; failed: MP4"
        );
        assert_eq!(
            bundle_recap(BundleKind::Video, &[ExportOp::Mp4, ExportOp::Webm]),
            "Exported nothing; failed: MP4, WebM"
        );
    }

    #[test]
    fn single_and_failed_recap_wording() {
        assert_eq!(single_recap("SVG", "./export/svg/"), "Exported SVG → ./export/svg/");
        assert_eq!(failed_recap(".piv v5"), ".piv v5 export failed");
    }

    #[test]
    fn export_failed_reads_the_core_status_convention() {
        let mut core = EditorCore::new();
        core.status = "Exported 5 PNG frames to ./export".into();
        assert!(!export_failed(&core));
        core.status = "MP4 export failed: ffmpeg not found".into();
        assert!(export_failed(&core));
        core.status = "Spritesheet failed: no frames".into();
        assert!(export_failed(&core));
    }

    #[test]
    fn op_destinations_match_the_row_notes() {
        // JPEG and APNG are the two formats that write to
        // out/ instead of ./export/ — the recaps must not
        // claim otherwise.
        assert_eq!(ExportOp::Jpeg.dest(), "out/jpeg/");
        assert_eq!(ExportOp::Apng.dest(), "out/animation.apng");
        assert_eq!(ExportOp::Png.dest(), "./export/");
        assert_eq!(ExportOp::Spritesheet.dest(), "./export/spritesheet.png");
    }
}
