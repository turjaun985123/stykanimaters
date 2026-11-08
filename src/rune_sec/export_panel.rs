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
//! Panel state: the toggle position lives in
//! `State::export_use_range` (see the integration report —
//! `main.rs` owns `State`). The range itself is the core's
//! real `loop_range`, set in the timeline; this panel only
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
/// guidance note — the GPUI `xc_row` shape.
fn plain_row(
    key: &'static str,
    label: &'static str,
    note: &'static str,
    t: &Theme,
    op: fn(&mut EditorCore),
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(btn(key, label, false, t, move |s: &mut State| {
            op(&mut s.core)
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

/// The export panel: header, the loop-range note (when a
/// range is set), the All frames / Loop range toggle with
/// its effective-behaviour line, then the four category
/// sections with all 15 `xc-*` rows.
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
        ));
    panel = panel.child(images);

    let animation = section("Animation", t)
        .child(plain_row(
            "xc-sheet",
            "Spritesheet",
            "All frames on one sheet — ./export/spritesheet.png",
            t,
            EditorCore::export_sheet,
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
        ))
        .child(plain_row(
            "xc-v3",
            ".piv v3",
            "Pivot v3 project — ./export/animation.piv",
            t,
            EditorCore::export_piv,
        ))
        .child(plain_row(
            "xc-v4",
            ".piv v4",
            "Pivot v4 project — ./export/animation-v4.piv",
            t,
            EditorCore::export_piv4,
        ))
        .child(plain_row(
            "xc-v5",
            ".piv v5",
            "Pivot v5 project — out/animation-v5.piv (out/, not export/)",
            t,
            EditorCore::export_v5_file,
        ))
        .child(plain_row(
            "xc-stk",
            ".stk",
            "Selected figure only — ./export/<figure>.stk",
            t,
            EditorCore::export_stk,
        ))
        .child(plain_row(
            "xc-nodes",
            ".nodes",
            "Selected figure for Stick Nodes — ./export/<figure>.nodes",
            t,
            EditorCore::export_nodes,
        ))
        .child(plain_row(
            "xc-stknds",
            ".stknds",
            "Whole project for Stick Nodes — ./export/animation.stknds",
            t,
            EditorCore::export_stknds,
        ));
    panel = panel.child(formats);

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
}
