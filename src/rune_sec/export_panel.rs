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
//! called. Nine formats also have frame-range exporters in
//! `export.rs` (PNG/JPEG/SVG, the spritesheet, the contact
//! sheet, GIF/APNG/MP4/WebM — the same free functions the
//! MCP layer calls, e.g. `mcp_io.rs`'s
//! `export_mp4_range`); when the panel's range toggle is set
//! to "Loop range" and a loop range exists, those nine write
//! only the loop-range frames, to the same destinations the
//! full ops use, and report through `core.status` in the
//! full ops' wording. Only the project formats have no
//! range variant, so they always export in full — the
//! toggle line says so rather than pretending
//! otherwise. MP4/WebM range exports are video-only (the
//! full MP4 export muxes audio; the range path cannot), and
//! the notes say that too.
//!
//! Phase M adds one more note, deliberately conditional:
//! when the range toggle is on and the project has a clip
//! that would actually be heard in a full export (unmuted,
//! file present on disk), a warning under the toggle row
//! (key `xc-range-audio-note`, decision in
//! `range_audio_note_shown`) states that range video
//! exports are silent — a range MP4/WebM otherwise loses
//! the project's sound with nothing saying so.
//!
//! Two bundle buttons sit under the categories: "Export
//! all images" (PNG + JPEG + SVG + spritesheet) and "Export
//! all video" (MP4 + WebM). A bundle is a composition of the
//! same full `EditorCore` ops the single-format buttons call
//! in "All frames" mode, run in `plan_bundle` order. Bundles
//! deliberately ignore the range toggle: every bundle
//! member has a range exporter, but a bundle that silently
//! wrote only the loop-range frames would not match its
//! "Export all" label — the bundle rows' notes say
//! "always export in full" instead. Each op sets
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
//! Under the recap sits a "Recent exports" list: the
//! newest files in the two roots the panel writes to
//! (`./export/` and `out/` — see `history_roots`), newest
//! first, at most eight, each row a name, a size and a
//! relative age, or a single "No exports yet" line when
//! there is nothing to show. It is a read-only glance at
//! what is on disk — there are deliberately no open/reveal
//! buttons, because the shell has no file opener to call.
//! The scan is cached behind the roots' mtimes (the
//! fx_browser/home pattern, see `recent_exports_cached`)
//! and lists only the top level of each root, so the
//! per-frame files inside `export/svg/` and `out/jpeg/`
//! do not appear individually.
//!
//! Panel state: the toggle position lives in
//! `State::export_use_range` and the recap line in
//! `State::export_recap` (see the integration report —
//! `main.rs` owns `State`; the integrator adds
//! `pub export_recap: String`, initialised to
//! `String::new()`). The range itself is the core's real
//! `loop_range`, set in the timeline; this panel only
//! reads it.
//!
//! Phase H gives the panel Font Awesome glyphs without
//! changing any key, label or handler: the Images /
//! Animation / Video category headings lead with an
//! `image` / `film` / `video` glyph and the Recent
//! exports heading with a `clock` (via
//! `icons::icon_el`, the frame_ops/fx_browser
//! inline-glyph idiom — 12 pt, secondary colour, leading
//! its row), and the media-format and bundle buttons
//! render through `icon_btn::icon_btn` with a glyph per
//! format type (`image` for PNG/JPEG/SVG and the images
//! bundle, `table-cells-large` for the spritesheet,
//! `film` for GIF/APNG, `video` for MP4/WebM and the
//! video bundle). The range toggle keeps its plain text
//! buttons — its active state is the text-button state —
//! and the project-format rows keep plain `btn`: no
//! glyph is assigned to them.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::icons::icon_el;
use crate::State;

/// The `xc-*` keys whose formats have a frame-range exporter
/// in `export.rs`. The project formats always export in
/// full.
pub const RANGE_KEYS: [&str; 9] = ["xc-png", "xc-jpeg", "xc-gif", "xc-apng", "xc-mp4", "xc-webm", "xc-contact", "xc-svg", "xc-sheet"];

/// Does the format behind this `xc-*` key support frame-range
/// export? (PNG/JPEG sequences, SVG, the spritesheet, the
/// contact sheet, GIF, APNG, MP4, WebM — the `*_range`
/// functions in `export.rs`.)
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
        "Loop range: frames {}–{} — with \"Loop range\" selected below, the PNG/JPEG/SVG/spritesheet/contact sheet/GIF/APNG/MP4/WebM exports write only those frames (MP4/WebM range exports are video-only).",
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

/// The exact wording of the range-mode audio warning
/// (element key `xc-range-audio-note`).
pub const RANGE_AUDIO_NOTE: &str =
    "Range video exports are silent — audio is only in full-timeline MP4/WebM.";

/// Whether the range-mode audio warning shows: the range
/// toggle is on AND at least one clip would actually be
/// heard in a full export — unmuted (`AudioClip::muted`)
/// and its file present on disk. The audibility half
/// mirrors the exclusions in `audio::playback_plan` (a
/// muted clip never sounds, and a clip whose file is
/// missing is dropped from the plan). The plan itself
/// probes files with `read_wav`; no dedicated existence
/// helper exists in the panel/waveform code, so the check
/// here is the plain `Path::exists` on the clip path —
/// the warning's question is only "is there a file the
/// full export would use", not "does it decode". Pure
/// apart from that one stat per clip.
pub fn range_audio_note_shown(core: &EditorCore, use_range: bool) -> bool {
    if !use_range {
        return false;
    }
    core.project
        .audio
        .iter()
        .any(|clip| !clip.muted && Path::new(&clip.path).exists())
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
    ContactSheet,
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
            ExportOp::ContactSheet => "contact sheet",
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
            ExportOp::ContactSheet => "./export/contact-sheet.png",
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
        ExportOp::ContactSheet => run_contact_full(&mut s.core),
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

/// SVG with the range toggle (Phase J): the range variant
/// writes into the same directory the full op uses —
/// `EditorCore::export_svg` writes to `./export/svg` — and
/// reports in the full op's wording with the frame range
/// named.
fn export_svg_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir().join("svg");
            let _ = std::fs::create_dir_all(&dir);
            match stykanimate_rs::export::export_svg_range(&s.core.project, a, b, &dir) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n} SVG frames (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        dir.display()
                    )
                }
                Err(e) => s.core.status = format!("SVG export failed: {e}"),
            }
        }
        None => s.core.export_svg(),
    }
    note_single(s, ExportOp::Svg);
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

/// Spritesheet with the range toggle (Phase J): the range
/// variant writes the one sheet to the full op's
/// destination (`./export/spritesheet.png`) and reports in
/// the contact-sheet action's voice — frame count, the
/// range, the path.
fn export_sheet_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("spritesheet.png");
            match stykanimate_rs::export::export_sheet_range(&s.core.project, a, b, &path) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n}-frame spritesheet (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        path.display()
                    )
                }
                Err(e) => s.core.status = format!("Spritesheet export failed: {e}"),
            }
        }
        None => s.core.export_sheet(),
    }
    note_single(s, ExportOp::Spritesheet);
}

/// The full contact-sheet export (Phase I, integrator
/// wiring for export.rs's range pair): unlike the other
/// formats there is no `EditorCore` op — the lib functions
/// write the file and this helper sets the status in the
/// family's voice (failure wording contains "failed", the
/// `export_failed` signal).
fn run_contact_full(core: &mut EditorCore) {
    let dir = export_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("contact-sheet.png");
    match stykanimate_rs::export::export_contact_sheet(&core.project, &path) {
        Ok(_) => {
            core.status = format!("Exported contact sheet to {}", path.display())
        }
        Err(e) => core.status = format!("Contact sheet export failed: {e}"),
    }
}

fn export_contact_action(s: &mut State) {
    match range_target(&s.core, s.export_use_range) {
        Some((a, b)) => {
            let dir = export_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("contact-sheet.png");
            match stykanimate_rs::export::export_contact_sheet_range(&s.core.project, a, b, &path) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n}-frame contact sheet (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        path.display()
                    )
                }
                Err(e) => s.core.status = format!("Contact sheet export failed: {e}"),
            }
        }
        None => run_contact_full(&mut s.core),
    }
    note_single(s, ExportOp::ContactSheet);
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

// ---- Recent exports history -------------------------------
// The newest files in the two output roots, as a read-only
// list under the recap. The row model and its formatting
// are pure; the filesystem touch is one thin scan, cached
// behind the roots' mtimes.

/// How many rows the Recent exports list shows at most.
pub const HISTORY_LIMIT: usize = 8;

/// The two output roots every export in this panel writes
/// into: `./export/` (PNG sequence frames, SVG,
/// spritesheet, GIF, MP4, WebM, .piv v2–v4, .stk, .nodes,
/// .stknds) and `out/` (JPEG sequences, APNG, .piv v5) —
/// see `ExportOp::dest` and the range actions. The history
/// scan reads exactly these two directories, nothing else.
pub fn history_roots() -> Vec<PathBuf> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![base.join("export"), base.join("out")]
}

/// One row of the Recent exports list: a file an export
/// wrote, with its size in bytes and its modification
/// time. The display name keeps the output root as a
/// prefix ("export/animation.mp4", "out/animation.apng")
/// because the two roots are both in play and a bare file
/// name does not say which one the file came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportRow {
    pub name: String,
    pub size: u64,
    pub mtime: SystemTime,
}

/// One raw scan result — (path, size in bytes, mtime) —
/// the plain-data input the row model is built from, so
/// the model builders stay pure and testable.
pub type FileTriple = (PathBuf, u64, SystemTime);

/// The display name for a scanned file: the last two path
/// components ("export/animation.mp4") when a parent
/// directory name exists, the bare file name otherwise.
pub fn display_name(path: &Path) -> String {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match path.parent().and_then(|p| p.file_name()) {
        Some(dir) => format!("{}/{file}", dir.to_string_lossy()),
        None => file,
    }
}

/// Build the Recent exports rows from raw scan triples:
/// newest first (name ascending breaks mtime ties, so the
/// order is deterministic), capped at `HISTORY_LIMIT`.
pub fn rows_from_triples(triples: &[FileTriple]) -> Vec<ExportRow> {
    let mut rows: Vec<ExportRow> = triples
        .iter()
        .map(|(path, size, mtime)| ExportRow {
            name: display_name(path),
            size: *size,
            mtime: *mtime,
        })
        .collect();
    rows.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(HISTORY_LIMIT);
    rows
}

/// A file size for the history rows: plain bytes under
/// 1 KB, then one-decimal KB / MB / GB.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
}

/// How long ago a history file was written, in the
/// coarsest bucket that fits: "just now" under a minute
/// (and for future mtimes — clock skew is not the user's
/// problem to read), then whole minutes, hours, days.
pub fn age_label(now: SystemTime, mtime: SystemTime) -> String {
    let secs = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);
    if secs < 60 {
        "just now".into()
    } else if secs < 60 * 60 {
        format!("{} min ago", secs / 60)
    } else if secs < 24 * 60 * 60 {
        format!("{} h ago", secs / (60 * 60))
    } else {
        format!("{} d ago", secs / (24 * 60 * 60))
    }
}

/// One Recent exports line: name, size, age.
pub fn row_line(row: &ExportRow, now: SystemTime) -> String {
    format!(
        "{} · {} · {}",
        row.name,
        format_size(row.size),
        age_label(now, row.mtime)
    )
}

/// The thin filesystem half of the history: read the top
/// level of each root and collect a `FileTriple` for every
/// regular file in it. Missing/unreadable roots and
/// entries whose metadata fails are skipped — an absent
/// export directory is the normal "nothing exported yet"
/// state, not an error. Subdirectories (`export/svg/`,
/// `out/jpeg/`) are not entered: their per-frame files are
/// not listed individually (see the module docs).
fn scan_triples(roots: &[PathBuf]) -> Vec<FileTriple> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(md) = entry.metadata() else {
                continue;
            };
            if !md.is_file() {
                continue;
            }
            let Ok(mtime) = md.modified() else {
                continue;
            };
            out.push((entry.path(), md.len(), mtime));
        }
    }
    out
}

/// A directory's modification time, swallowing every
/// error into `None` — the fx_browser/home_panel helper,
/// same rule: a missing/unreadable dir is a state, not a
/// failure. Callers treat `None` as "no trustworthy cache
/// key", not as a key.
fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// One history-cache slot: the roots' mtimes the rows were
/// scanned under, plus the rows themselves.
type HistoryCache = Option<(Vec<Option<SystemTime>>, Vec<ExportRow>)>;

thread_local! {
    /// The Recent exports cache. Thread-local for the same
    /// reasons as the fx preset cache: Rune builds the view
    /// on the UI thread (so an export press and the next
    /// build share one cache), and every test thread then
    /// starts with an empty cache.
    static HISTORY_CACHE: RefCell<HistoryCache> = RefCell::new(None);
}

/// The cache state machine, pure over its inputs (the
/// fx_browser `cached_names` shape): serve the stored rows
/// when `key` matches the key they were scanned under,
/// otherwise call `read`, store its result under `key`,
/// and return it. The "unstattable roots bypass the cache"
/// policy lives in [`recent_exports_cached`], not here.
fn cached_history(
    cache: &mut HistoryCache,
    key: Vec<Option<SystemTime>>,
    read: impl FnOnce() -> Vec<ExportRow>,
) -> Vec<ExportRow> {
    if let Some((cached_key, rows)) = cache.as_ref() {
        if *cached_key == key {
            return rows.clone();
        }
    }
    let rows = read();
    *cache = Some((key, rows.clone()));
    rows
}

/// The Recent exports rows, cached behind the two roots'
/// mtimes (the Phase E fx_browser/home scan pattern):
/// building the panel costs two `stat`s instead of two
/// directory scans, and any export that creates or deletes
/// a file in a root bumps that root's mtime, so the next
/// build rescans. The honest limit, the same class as the
/// preset cache's documented contract: overwriting an
/// existing file in place (re-exporting the same format
/// to the same name) changes neither root's mtime, so
/// that row's size/age can be one scan stale until some
/// create/delete lands in a root. When neither root can
/// be statted (no exports yet, usually) the cache is
/// bypassed and the scan runs live — it returns empty,
/// which the panel renders as "No exports yet".
pub fn recent_exports_cached() -> Vec<ExportRow> {
    let roots = history_roots();
    let keys: Vec<Option<SystemTime>> = roots.iter().map(|d| dir_mtime(d)).collect();
    if keys.iter().all(|k| k.is_none()) {
        return rows_from_triples(&scan_triples(&roots));
    }
    HISTORY_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        cached_history(&mut slot, keys, || rows_from_triples(&scan_triples(&roots)))
    })
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

/// A section column whose heading leads with a Font
/// Awesome glyph (Phase H): the same [`section`] label —
/// same font, same colour — preceded by a 12 pt
/// [`icon_el`] in the heading's secondary colour, in a
/// centred row: the inline-glyph idiom frame_ops and
/// fx_browser use. Only the heading changes; the column
/// below it is built exactly as [`section`] builds it.
fn section_with_icon(icon: &str, title: &str, t: &Theme) -> El {
    col().gap(6.0).child(
        row()
            .gap(6.0)
            .center_y()
            .child(icon_el(icon, 12.0, t.text_secondary))
            .child(text(title).font(type_::SECTION).color(t.text_secondary)),
    )
}

/// One format row for a full-export-only format: an `xc-`
/// button calling the `EditorCore` op, plus its muted
/// guidance note — the GPUI `xc_row` shape. The handler
/// also records the panel recap (`recap_name` +
/// `recap_dest`, or a failure note when the op reports
/// one — see `note_single`). With `icon` set (Phase H)
/// the button renders through `icon_btn` with that glyph
/// before the label; without one it is the plain `btn` —
/// same key, same label, same handler either way.
fn plain_row(
    key: &'static str,
    icon: Option<&str>,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    op: fn(&mut EditorCore),
    recap_name: &'static str,
    recap_dest: &'static str,
) -> El {
    let handler = move |s: &mut State| {
        op(&mut s.core);
        s.export_recap = if export_failed(&s.core) {
            failed_recap(recap_name)
        } else {
            single_recap(recap_name, recap_dest)
        };
    };
    row()
        .gap(8.0)
        .center_y()
        .child(match icon {
            Some(icon) => icon_btn(key, icon, label, false, t, handler),
            None => btn(key, label, false, t, handler),
        })
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// One format row for a range-capable format: the button
/// (an `icon_btn` with the format type's glyph before the
/// label — Phase H; same key, same label, same handler)
/// is one of the `export_*_action` functions above, which
/// choose between the range exporter and the full op at
/// press time.
fn range_row(
    key: &'static str,
    icon: &str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    action: fn(&mut State),
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(icon_btn(key, icon, label, false, t, action))
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// One bundle row: an `xc-bundle-*` `icon_btn` (the
/// bundle type's glyph before the label — Phase H; same
/// key, same label, same handler) running `run_bundle`
/// for its kind, plus the note stating that bundles
/// always export in full (the range toggle governs only
/// the single-format buttons).
fn bundle_row(
    key: &'static str,
    icon: &str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    kind: BundleKind,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(icon_btn(key, icon, label, false, t, move |s: &mut State| {
            run_bundle(s, kind)
        }))
        .child(text(note).font(type_::LABEL).color(t.text_secondary))
}

/// The export panel: header, the loop-range note (when a
/// range is set), the All frames / Loop range toggle with
/// its effective-behaviour line and (when it applies) the
/// range audio warning, the four category sections
/// with all 16 `xc-*` rows, the Bundles section (two
/// `xc-bundle-*` rows), — once anything has been exported —
/// the recap line showing the last export, and the Recent
/// exports list closing the panel.
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
    if range_audio_note_shown(core, use_range) {
        panel = panel.child(
            text(RANGE_AUDIO_NOTE)
                .key("xc-range-audio-note")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    panel = panel.child(
        text(effective_text(core, use_range))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    let images = section_with_icon("image", "Images", t)
        .child(range_row(
            "xc-png",
            "image",
            "PNG",
            "PNG image sequence — one file per frame in ./export/",
            t,
            export_png_action,
        ))
        .child(range_row(
            "xc-jpeg",
            "image",
            "JPEG",
            "JPEG image sequence in out/jpeg/ (out/, not export/)",
            t,
            export_jpeg_action,
        ))
        .child(range_row(
            "xc-svg",
            "image",
            "SVG",
            "One SVG per frame in ./export/svg/",
            t,
            export_svg_action,
        ));
    panel = panel.child(images);

    let animation = section_with_icon("film", "Animation", t)
        .child(range_row(
            "xc-sheet",
            "table-cells-large",
            "Spritesheet",
            "All frames on one sheet — ./export/spritesheet.png",
            t,
            export_sheet_action,
        ))
        .child(range_row(
            "xc-contact",
            "table-cells-large",
            "Contact sheet",
            "Every frame as one tile sheet — ./export/contact-sheet.png",
            t,
            export_contact_action,
        ))
        .child(range_row(
            "xc-gif",
            "film",
            "GIF",
            "Animated GIF — ./export/animation.gif",
            t,
            export_gif_action,
        ))
        .child(range_row(
            "xc-apng",
            "film",
            "APNG",
            "Animated PNG — out/animation.apng (out/, not export/)",
            t,
            export_apng_action,
        ));
    panel = panel.child(animation);

    let video = section_with_icon("video", "Video", t)
        .child(range_row(
            "xc-mp4",
            "video",
            "MP4",
            "MP4 video via ffmpeg — ./export/animation.mp4 (full export includes audio; range export is video-only)",
            t,
            export_mp4_action,
        ))
        .child(range_row(
            "xc-webm",
            "video",
            "WebM",
            "WebM video (VP9) — ./export/animation.webm",
            t,
            export_webm_action,
        ));
    panel = panel.child(video);

    let formats = section("Project formats", t)
        .child(plain_row(
            "xc-v2",
            None,
            ".piv v2",
            "Pivot v2 project — ./export/animation-v2.piv",
            t,
            EditorCore::export_piv2,
            ".piv v2",
            "./export/animation-v2.piv",
        ))
        .child(plain_row(
            "xc-v3",
            None,
            ".piv v3",
            "Pivot v3 project — ./export/animation.piv",
            t,
            EditorCore::export_piv,
            ".piv v3",
            "./export/animation.piv",
        ))
        .child(plain_row(
            "xc-v4",
            None,
            ".piv v4",
            "Pivot v4 project — ./export/animation-v4.piv",
            t,
            EditorCore::export_piv4,
            ".piv v4",
            "./export/animation-v4.piv",
        ))
        .child(plain_row(
            "xc-v5",
            None,
            ".piv v5",
            "Pivot v5 project — out/animation-v5.piv (out/, not export/)",
            t,
            EditorCore::export_v5_file,
            ".piv v5",
            "out/animation-v5.piv",
        ))
        .child(plain_row(
            "xc-stk",
            None,
            ".stk",
            "Selected figure only — ./export/<figure>.stk",
            t,
            EditorCore::export_stk,
            ".stk",
            "./export/<figure>.stk",
        ))
        .child(plain_row(
            "xc-nodes",
            None,
            ".nodes",
            "Selected figure for Stick Nodes — ./export/<figure>.nodes",
            t,
            EditorCore::export_nodes,
            ".nodes",
            "./export/<figure>.nodes",
        ))
        .child(plain_row(
            "xc-stknds",
            None,
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
            "image",
            "Export all images",
            "PNG + JPEG + SVG + spritesheet in one press. Bundles always export in full — the range toggle above applies only to the single-format buttons.",
            t,
            BundleKind::Images,
        ))
        .child(bundle_row(
            "xc-bundle-video",
            "video",
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

    // Recent exports: what is actually on disk in the two
    // output roots, newest first — read-only rows, no
    // open/reveal buttons (the shell has no file opener).
    let mut recent = section_with_icon("clock", "Recent exports", t).key("xc-history");
    let history = recent_exports_cached();
    if history.is_empty() {
        recent = recent.child(
            text("No exports yet")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        let now = SystemTime::now();
        for r in &history {
            recent = recent.child(
                text(row_line(r, now))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
    }
    panel = panel.child(recent);

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_support_matches_the_export_rs_range_functions() {
        for key in [
            "xc-png", "xc-jpeg", "xc-gif", "xc-apng", "xc-mp4", "xc-webm", "xc-contact",
            "xc-svg", "xc-sheet",
        ] {
            assert!(supports_range(key), "{key} should support range export");
        }
        for key in [
            "xc-v2", "xc-v3", "xc-v4", "xc-v5", "xc-stk", "xc-nodes", "xc-stknds", "xc-bogus",
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

    // ---- Recent exports history ----

    /// A fixed clock for the history tests: `secs` after
    /// the Unix epoch.
    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
    }

    #[test]
    fn history_roots_are_the_two_output_directories() {
        let roots = history_roots();
        assert_eq!(roots.len(), 2);
        assert!(roots[0].ends_with("export"), "first root: {:?}", roots[0]);
        assert!(roots[1].ends_with("out"), "second root: {:?}", roots[1]);
    }

    #[test]
    fn format_size_uses_bytes_then_one_decimal_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(5 * 1024 * 1024 + 512 * 1024), "5.5 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn age_label_buckets_by_coarsest_unit() {
        let now = at(10 * 24 * 60 * 60);
        assert_eq!(age_label(now, now), "just now");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 59)), "just now");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 60)), "1 min ago");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 45 * 60)), "45 min ago");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 60 * 60)), "1 h ago");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 23 * 60 * 60)), "23 h ago");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 24 * 60 * 60)), "1 d ago");
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 - 3 * 24 * 60 * 60)), "3 d ago");
        // A future mtime (clock skew) reads as "just now",
        // never as a negative age.
        assert_eq!(age_label(now, at(10 * 24 * 60 * 60 + 3600)), "just now");
    }

    #[test]
    fn display_name_keeps_the_output_root_as_prefix() {
        assert_eq!(
            display_name(Path::new("/home/u/export/animation.mp4")),
            "export/animation.mp4"
        );
        assert_eq!(
            display_name(Path::new("/home/u/out/animation.apng")),
            "out/animation.apng"
        );
        assert_eq!(display_name(Path::new("animation.gif")), "animation.gif");
    }

    #[test]
    fn rows_from_triples_sorts_newest_first_with_name_tie_break() {
        let triples = vec![
            (PathBuf::from("/x/export/old.gif"), 10, at(100)),
            (PathBuf::from("/x/export/b.png"), 10, at(300)),
            (PathBuf::from("/x/out/a.apng"), 10, at(300)),
            (PathBuf::from("/x/export/new.mp4"), 10, at(500)),
        ];
        let rows = rows_from_triples(&triples);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["export/new.mp4", "export/b.png", "out/a.apng", "export/old.gif"]
        );
        // Sizes and mtimes ride along untouched.
        assert_eq!(rows[0].size, 10);
        assert_eq!(rows[0].mtime, at(500));
    }

    #[test]
    fn rows_from_triples_caps_at_the_history_limit() {
        let triples: Vec<FileTriple> = (0..12)
            .map(|i| (PathBuf::from(format!("/x/export/f{i}.png")), 1, at(1000 + i)))
            .collect();
        let rows = rows_from_triples(&triples);
        assert_eq!(rows.len(), HISTORY_LIMIT);
        assert_eq!(rows.first().unwrap().name, "export/f11.png", "newest kept");
        assert_eq!(rows.last().unwrap().name, "export/f4.png", "oldest four dropped");
    }

    #[test]
    fn rows_from_no_triples_is_empty() {
        // The empty scan — missing roots, or roots with no
        // files — is what the panel turns into its single
        // "No exports yet" line.
        assert!(rows_from_triples(&[]).is_empty());
    }

    #[test]
    fn row_line_combines_name_size_and_age() {
        let row = ExportRow {
            name: "export/animation.mp4".into(),
            size: 1536,
            mtime: at(1000),
        };
        assert_eq!(
            row_line(&row, at(1000 + 2 * 60 * 60)),
            "export/animation.mp4 · 1.5 KB · 2 h ago"
        );
    }

    #[test]
    fn cached_history_reads_once_per_key() {
        let mut cache: HistoryCache = None;
        let key = vec![Some(at(7)), None];
        let mut reads = 0;
        let first = cached_history(&mut cache, key.clone(), || {
            reads += 1;
            Vec::new()
        });
        let second = cached_history(&mut cache, key.clone(), || {
            reads += 1;
            Vec::new()
        });
        assert_eq!(first, second);
        assert_eq!(reads, 1, "same key must be served from the cache");
        let _ = cached_history(&mut cache, vec![Some(at(8)), None], || {
            reads += 1;
            Vec::new()
        });
        assert_eq!(reads, 2, "a changed root mtime must rescan");
    }

    // ---- Range audio warning (Phase M, section 32) ----

    /// A clip literal for the warning matrix.
    fn clip(path: &str, muted: bool) -> stykanimate_rs::model::AudioClip {
        stykanimate_rs::model::AudioClip {
            path: path.into(),
            start_frame: 0,
            gain: 1.0,
            muted,
        }
    }

    /// Create a real (empty) file in the temp dir and
    /// return its path; the caller removes it. The warning
    /// checks existence only, so the file needs no WAV
    /// content.
    fn existing_clip_file(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rs-xc-note-{tag}-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, b"").expect("create the temp clip file");
        path
    }

    /// A temp-dir path that is never created — the
    /// "file does not exist" fixture.
    fn missing_clip_path() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rs-xc-note-missing-{}.wav",
            std::process::id()
        ));
        assert!(!path.exists(), "fixture path must not exist: {path:?}");
        path
    }

    #[test]
    fn range_audio_note_wording_is_exact() {
        assert_eq!(
            RANGE_AUDIO_NOTE,
            "Range video exports are silent — audio is only in full-timeline MP4/WebM."
        );
    }

    #[test]
    fn range_audio_note_hidden_when_range_off_even_with_audible_clip() {
        let file = existing_clip_file("range-off");
        let mut core = EditorCore::new();
        core.project.audio.push(clip(&file.to_string_lossy(), false));
        assert!(!range_audio_note_shown(&core, false));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn range_audio_note_hidden_for_the_demo_project() {
        // The demo project ships with no audio clips at all
        // (`Project::new_demo`'s audio vec is empty), so
        // with the range toggle on there is nothing a full
        // export would carry — the note stays hidden.
        let core = EditorCore::new();
        assert!(core.project.audio.is_empty(), "demo premise");
        assert!(!range_audio_note_shown(&core, true));
    }

    #[test]
    fn range_audio_note_hidden_with_only_muted_clips() {
        // The clip's file exists; muting alone hides the
        // note, because a full export would not carry it.
        let file = existing_clip_file("muted");
        let mut core = EditorCore::new();
        core.project.audio.push(clip(&file.to_string_lossy(), true));
        assert!(!range_audio_note_shown(&core, true));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn range_audio_note_hidden_when_audible_clip_file_is_missing() {
        let missing = missing_clip_path();
        let mut core = EditorCore::new();
        core.project
            .audio
            .push(clip(&missing.to_string_lossy(), false));
        assert!(!range_audio_note_shown(&core, true));
    }

    #[test]
    fn range_audio_note_shown_with_an_audible_existing_clip() {
        let file = existing_clip_file("audible");
        let mut core = EditorCore::new();
        core.project.audio.push(clip(&file.to_string_lossy(), false));
        assert!(range_audio_note_shown(&core, true));
        let _ = std::fs::remove_file(&file);
    }
}
