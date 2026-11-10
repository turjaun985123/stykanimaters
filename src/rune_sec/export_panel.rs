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
//! otherwise. MP4 range exports mux the audio of the
//! clips that intersect the range (Wave 9 for WAV,
//! `export::export_mp4_range`; Wave 10 extended it to
//! any ffmpeg-readable format, and a clip whose file
//! cannot be probed or decoded fails the export rather
//! than being dropped silently); since Wave W
//! (W6) the WebM range exports mux the same range
//! audio as Opus (`export::export_webm_range`), while
//! the full WebM export stays video-only
//! (`export::export_webm` never muxes) — and the
//! notes say both.
//!
//! Phase M adds one more note, deliberately conditional:
//! when the range toggle is on and the project has a clip
//! that would actually be heard in a full export (unmuted,
//! file present on disk), a note under the toggle row
//! (key `xc-range-audio-note`, decision in
//! `range_audio_note_shown`) states what range mode does
//! with the sound: a range MP4 keeps the audio of clips
//! inside the range (WAV and, since Wave 10, any
//! ffmpeg-readable format; an unreadable clip file fails
//! the export rather than being dropped silently), and
//! since Wave W (W6) a range WebM keeps it too, as Opus
//! — so the coverage is said out loud instead of the
//! project's sound quietly changing shape. (A full WebM
//! export still never carries the sound; the WebM row's
//! own note says that where the button is.)
//!
//! Wave 12 adds one more conditional note, in the Project
//! formats group: when the project carries media a .piv
//! export would drop — image/text segments, which .piv
//! v2/v3/v4 write as plain segments (only v5 writes
//! segment media, through its side tables), or sprite /
//! text instances, which no .piv version writes at all —
//! a note under the .piv rows (key `xc-piv-media-note`,
//! decision in `piv_media_note`) says so where the buttons
//! are, instead of the export's bare success status being
//! the only word. The drops themselves are the writers'
//! documented behaviour (`pivot.rs`); the note only
//! repeats them at the point of export. Media-free
//! projects show nothing, so their panel is unchanged.
//!
//! Wave 13 adds the sprite half of that honesty for the
//! remaining project formats, under the figure-file rows
//! (key `xc-sprite-note`, decision in `sprite_note`):
//! when the project has sprite instances, a note says
//! they are left out of the .stk, .nodes and .stknds
//! exports. The writers prove it from their signatures
//! and bodies: `.stk` (`pivot::export_stk`) and `.nodes`
//! (`nodes::export_nodes_figure`) take a single
//! `FigureDef` — sprite instances are project-level, so
//! there is nowhere in either file for them to go — and
//! `.stknds` (`nodes_anim::export_nodes_animation`)
//! takes the whole project but writes its sprite
//! library and per-frame sprite counts as empty, so
//! the instances are dropped from a whole-project
//! export too. The .piv half of the sprite story is not
//! repeated here: the Wave 12 note above already names
//! it. Sprite-free projects show nothing, so their
//! panel is unchanged.
//!
//! Two bundle buttons sit under the categories: "Export
//! all images" (PNG + JPEG + SVG + spritesheet) and "Export
//! all video" (MP4 + WebM). A bundle is a composition of the
//! same full exports the single-format buttons run in "All
//! frames" mode, run in `plan_bundle` order. Bundles
//! deliberately ignore the range toggle: every bundle
//! member has a range exporter, but a bundle that silently
//! wrote only the loop-range frames would not match its
//! "Export all" label — the bundle rows' notes say
//! "always export in full" instead. Since Wave Z (Z8) a
//! bundle runs as a background job like the contracted
//! single formats (see the "Export jobs" section below):
//! the worker runs each member's hooked entry over a
//! cloned project — the plain entries the `EditorCore`
//! ops call are those entries with no callback — under
//! the core bundle-progress tracker (Wave X, X11), and
//! when the job finishes, the finish path writes one
//! recap line to the status and the panel recap (built
//! by `bundle_recap` from the members actually run,
//! naming any that failed) — the same last word the
//! synchronous handler had.
//!
//! The recap line under the bundles shows the last export —
//! bundle or single — as "Exported <name> → <destination>".
//! A range single names its frame range in the recap (and
//! video-only for WebM) rather than claiming the plain
//! full destination, and a failure keeps the op's own
//! status wording, reason included, instead of dropping
//! to a bare "<name> export failed". The line renders in
//! the toast severity palette (glyph + colour from
//! `recap_severity`), so the record that persists after
//! the toast fades still says how the export went.
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
//! fx_browser/home pattern, see `recent_exports_cached`).
//! Sequences never flood it: the `frame-*.png` files a
//! PNG export writes straight into a root collapse into
//! one row ("export/frame-*.png · N files"), and the two
//! sequence subdirectories — `export/svg/`, `out/jpeg/` —
//! each appear as a single row carrying the directory's
//! mtime and the files' total size, instead of the
//! per-frame files inside them (or, as before, nothing
//! at all).
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
//!
//! Phase N restyles the panel's shell without touching
//! a key, label or handler: the root is the kit card
//! (`design::panel_col`, key `export-panel` as ever),
//! the header is a kit `panel_title` over a hairline
//! `divider` with the subtitle kept as the first body
//! line beneath it, and the private section helpers
//! are retired — every category group is now a plain
//! column headed by `design::section_label`, the icon
//! variant keeping its local glyph + label row.
//!
//! Wave 9 (UI mission, polish pass) fixes three proven
//! defects without touching a key, label or handler:
//! (1) Grouping: the spritesheet and contact sheet are
//! single-image outputs, and the images bundle already
//! treats the spritesheet as an image — yet both rows
//! sat under "Animation", beside the two true animated
//! formats (GIF, APNG). Both rows now sit under
//! "Images", so each group has one definition: still
//! images, animated images, video, project files. The
//! contact sheet is still not a bundle member (bundles
//! are a behaviour contract, not a grouping one — see
//! the plan docs above). (2) Range honesty: the two
//! sheet notes claimed "All frames" / "Every frame",
//! which is false whenever the loop-range toggle is
//! active; they now read "Frames …", true in both
//! modes. (3) Containment: every guidance note and
//! free-text line in the panel was a plain unwrapped
//! `text()` — Rune text does not wrap unless `.wrap()`
//! is set, so any line wider than the 316 pt card
//! content spilled past its edge (the Wave 2
//! `empty_state` class; cf. the audio panel's caption,
//! 144 chars ≈ 880 pt). At kit LABEL metrics
//! (≈6 pt/char) the MP4 note measures ≈612 pt, the
//! bundle notes ≈850–970 pt, and the loop-range note
//! is longer still — all spilled on every render. Row
//! notes now `.wrap().grow(1.0)` beside their button
//! (the status-strip tail idiom), and the free-text
//! lines (loop note, effective line, range audio
//! warning, recent-export rows) take the kit prose
//! treatment `.wrap().w_full()`; the recap line wraps
//! and grows beside its severity glyph, so a long
//! failure reason wraps instead of spilling. Lines
//! that provably fit (the 42-char subtitle, the
//! bounded output line) are unchanged.
//!
//! Wave R (R5) wires the panel's labels to the i18n
//! catalogue through `i18n_wire::tr` — the shell's one
//! established idiom — wherever a catalogue key's
//! English value is byte-identical to the label the
//! panel already showed: the title (`ws_export`), the
//! range toggle's caption (`frames`, with its colon
//! composed on), and the SVG / spritesheet / WebM
//! buttons (`export_svg`, `spritesheet`, `export_webm`).
//! English rendering is therefore unchanged (pinned in
//! the tests); other languages now localise those five
//! labels.
//!
//! Wave S (S1) closes R5's no-key gaps: the catalogue
//! gained one block of panel-shaped keys (the section
//! headings `export_sec_*`, `export_jpeg`,
//! `export_apng`, `export_contact_sheet`,
//! `export_nodes`, `export_stknds`,
//! `export_all_frames`, the bundle buttons
//! `export_bundle_images` / `export_bundle_video`, the
//! history empty state `export_no_exports_yet` and the
//! header subtitle `export_subtitle`), and every one
//! of those labels now renders through `tr` with
//! English byte-identical. The labels that sit next to
//! a key whose value is a DIFFERENT string stay
//! hardcoded by the same rule (`export_png` is
//! "Export PNG", the button is "PNG"; likewise the
//! GIF / MP4 / .stk buttons and "Loop range" beside
//! `export_gif` / `export_mp4` / `export_stk` /
//! `hud_loop`) — the discrepancies are filed in the
//! Wave S findings (s1.md) for a catalogue decision,
//! neither side changed here. The guidance notes and
//! the composed prose lines (loop note, effective /
//! output lines, recaps, history rows) are
//! sentence-length copy with no catalogue keys; they
//! stay English, the Q14 blurb precedent. No element
//! key, handler or layout changed.
//!
//! Wave T (T1) wires the class-C remainder the R5/S1
//! passes filed: every row guidance note, the composed
//! prose lines (loop note, effective / output lines,
//! the range audio note, the .piv media note, the
//! sprite note) and the Recent exports row text (age
//! buckets and the row templates) now render through
//! `tr` — the prose lines with `{slot}` templates in
//! the camera `follow_label` idiom, replaced at the
//! call site — with English byte-identical (the new
//! keys are IA-T1-1, inserted by the integrator after
//! the `threed_*` block; until they land the wire falls
//! back to the key text, the Q7 pattern, and the i18n
//! pin says so). Two classes stay English by design,
//! filed in the Wave T findings: the range actions'
//! `core.status` sentences and the recap builders'
//! lines are machine-read records, not display copy —
//! `export_failed` detects failure by the status's
//! "failed" substring, `recap_severity` classifies the
//! stored recap by `classify_status`'s English shapes
//! ("Exported " prefix, " failed"), and the status
//! sentences mirror the `editor.rs` full ops' wording,
//! which stays English; translating this panel's half
//! would break failure detection and severity in every
//! other language. `ExportOp::name()` likewise stays:
//! its values are the `{name}` slot data the recaps
//! interpolate (format names, plus "sheet" / "contact
//! sheet" riding as data), not label sites. No element
//! key, handler or layout changed.
//!
//! Wave W (W1) moves the four formats with a boarded
//! progress contract — PNG/JPEG sequences (the render
//! phase, IA-V12-1) and MP4/WebM (the encode phase,
//! IA-U11-1; since W2's funnel adoption the video
//! entries report their render loop too) — onto a
//! background job (see the "Export jobs" section
//! below): the press spawns a worker over a cloned
//! project, the contracts' callbacks write into two
//! separate atomic phase slots, never one merged
//! figure, and the shell's frame tick (`main::poll` →
//! [`poll_export_job`]) drains the job — repainting on
//! snapshot changes and, on completion, writing the
//! status and recap with the exact sentences the
//! synchronous paths produced, class-C wording
//! included. Progress text never enters `core.status`;
//! the readout's lines live under the recap, keyed
//! `xc-progress*`, and every number in them is a stored
//! contract event — a phase that has not reported
//! shows no number. While a job runs, every export
//! press in the panel is a no-op: one export at a
//! time, the contract the blocking calls enforced by
//! construction. The formats with no phase contract
//! (SVG, GIF, APNG, sheet, contact and the project
//! formats) keep their synchronous paths. The readout's
//! labels ride IA-W1-1
//! (`export_progress_working`, `export_render_progress`,
//! `export_encode_progress`, `export_encode_finished`);
//! until the integrator inserts them the wire falls
//! back to the key text, the Q7 pattern.
//!
//! Wave Z (Z8) puts the two bundles on the same job
//! mechanism: a bundle press spawns a worker that runs
//! the plan's members in order over the cloned project,
//! each member through the hooked entry whose plain
//! form the member's `EditorCore` op calls, under the
//! core `BundleProgress` tracker (Wave X, X11) — one
//! combined count, its total the sum of the members'
//! totals allocated before the first member runs, so
//! the count can never reset or run backwards at a
//! member boundary. The tracker's events write into
//! the job's third slot — the bundle slot, kept apart
//! from the two phase slots exactly as the phases are
//! kept apart from each other: a bundle count is its
//! own core contract, not a phase, and no code path
//! adds it to a phase counter. The readout shows the
//! one bundle line (`xc-progress-bundle`, IA-Z8-1's
//! `export_bundle_progress`); the SVG and spritesheet
//! members have no phase contract of their own, so
//! during them the line holds at the last true count
//! and their frames land when they complete — the
//! tracker's documented behaviour, not an estimate.
//! The finish writes the bundle recap to the status
//! and the recap together, as the synchronous handler
//! did; a member's failure never stops the bundle —
//! every member is attempted, and the recap names the
//! failures, exactly as before.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::icons::{icon_el, icon_el_rgb};
use crate::rune_sec::toasts::{classify_status, Severity};
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
/// one toggle away, and a range MP4 keeps the audio of the
/// clips inside the range (WAV, and since Wave 10 any
/// ffmpeg-readable format; an unprobeable clip file fails
/// the export) — and since Wave W (W6) a range WebM keeps
/// it too, as Opus; only a full WebM export stays silent.
pub fn loop_note(core: &EditorCore) -> Option<String> {
    let (a, b) = range_target(core, true)?;
    // Wave T (T1): the sentence is a catalogue template
    // (`export_loop_note`, IA-T1-1) with `{from}` / `{to}`
    // slots, the camera `follow_label` idiom; in English
    // the composed line is byte-identical to the pre-wire
    // literal.
    Some(
        i18n_wire::tr(core, "export_loop_note")
            .replace("{from}", &(a + 1).to_string())
            .replace("{to}", &(b + 1).to_string()),
    )
}

/// The line under the range toggle stating what the next
/// export will actually cover, in every toggle/range
/// combination — including the honest fallback when "Loop
/// range" is selected but no loop range exists.
pub fn effective_text(core: &EditorCore, use_range: bool) -> String {
    let n = core.project.frames.len();
    if n == 0 {
        return i18n_wire::tr(core, "export_effective_none");
    }
    // Wave T (T1): each branch is a catalogue string
    // (IA-T1-1) — the two range branches are `{from}` /
    // `{to}` / `{count}` templates composed here, the
    // camera `follow_label` idiom; English output is
    // byte-identical to the pre-wire literals.
    match (use_range, range_target(core, true)) {
        (true, Some((a, b))) => i18n_wire::tr(core, "export_effective_range")
            .replace("{from}", &(a + 1).to_string())
            .replace("{to}", &(b + 1).to_string()),
        (true, None) => i18n_wire::tr(core, "export_effective_no_range"),
        (false, _) => {
            i18n_wire::tr(core, "export_effective_all").replace("{count}", &n.to_string())
        }
    }
}

/// A stage dimension or fps for the output line: whole
/// values print without a decimal point ("640", "12"),
/// fractional ones with one decimal ("29.9") — the
/// status line's fps convention.
fn fmt_measure(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

/// The output summary under the toggle: what the next
/// export will actually produce — the frame count the
/// toggle state implies (the loop-range count when a
/// range target applies, every frame otherwise), the
/// stage dimensions every export renders at (there is no
/// scale control anywhere in `export.rs`; output size IS
/// `project.stage`), and the project fps. Driven by the
/// same state `effective_text` reads, so the two lines
/// can never disagree about the frame count.
pub fn output_text(core: &EditorCore, use_range: bool) -> String {
    let frames = match range_target(core, use_range) {
        Some((a, b)) => b - a + 1,
        None => core.project.frames.len(),
    };
    let [w, h] = core.project.stage;
    // Wave T (T1): a catalogue template (`export_output`,
    // IA-T1-1) with `{frames}` / `{width}` / `{height}` /
    // `{fps}` slots; the measures keep this panel's
    // `fmt_measure` formatting, and the English line is
    // byte-identical to the pre-wire format.
    i18n_wire::tr(core, "export_output")
        .replace("{frames}", &frames.to_string())
        .replace("{width}", &fmt_measure(w))
        .replace("{height}", &fmt_measure(h))
        .replace("{fps}", &fmt_measure(core.project.fps))
}

/// The exact wording of the range-mode audio note
/// (element key `xc-range-audio-note`). Since Wave 9 a
/// range MP4 muxes the audio of WAV clips that intersect
/// the range (`export::export_mp4_range`); since Wave 10
/// non-WAV clips are cut to the range the same way (the
/// engine probes them with ffprobe and decodes the
/// segment through ffmpeg), and a clip whose file cannot
/// be probed or decoded fails the export outright rather
/// than being dropped silently. Since Wave W (W6) the
/// range WebM exports mux the same range audio as Opus
/// (`export::export_webm_range`), so the note now names
/// both formats; only the full WebM export stays silent.
/// Wave T (T1): the panel renders the catalogue key
/// `export_range_audio_note` (IA-T1-1), whose English
/// value is this constant — the constant stays as the
/// canonical English wording, and the i18n pin holds
/// the two equal. Wave W (W6): the new English value
/// reaches the catalogue through IA-W6-1.
pub const RANGE_AUDIO_NOTE: &str =
    "Range MP4 and WebM exports include audio from WAV and other ffmpeg-readable clips cut to the range — a clip whose file cannot be probed or decoded fails the export instead of being left out silently.";

/// Whether the range-mode audio note shows: the range
/// toggle is on AND at least one clip would actually be
/// heard in a full export — unmuted (`AudioClip::muted`)
/// and its file present on disk. The audibility half
/// mirrors the exclusions in `audio::playback_plan` (a
/// muted clip never sounds, and a clip whose file is
/// missing is dropped from the plan). The plan itself
/// probes files with `read_wav`; no dedicated existence
/// helper exists in the panel/waveform code, so the check
/// here is the plain `Path::exists` on the clip path —
/// the note's question is only "is there a file the
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

/// Does any figure carry segment media — a node with an
/// image or text segment (`NodeDef::media`)? Pure: reads
/// the project only.
pub fn project_has_segment_media(core: &EditorCore) -> bool {
    core.project
        .figures
        .iter()
        .any(|fig| fig.def.nodes.iter().any(|node| node.media.is_some()))
}

/// The .piv media note (element key `xc-piv-media-note`),
/// shown under the .piv rows when the project carries
/// media a .piv export would drop. The Pivot writers
/// document the drops (`pivot.rs`): the shared figure
/// stream behind v2/v3/v4 stores geometry and shape only,
/// so image/text segments export as plain segments there;
/// v5 writes segment media (its image side tables hold
/// the referenced assets' PNG bytes, its text side tables
/// encode the bitmap glyphs). No .piv writer reads
/// `project.sprites` or `project.texts` at all — the
/// grammar has no free-floating instances — so sprite and
/// text instances are dropped by every version. Until now
/// the only word was the ops' bare success status; the
/// note says it before the export instead. Each clause
/// appears only when the project actually carries that
/// kind of media, so a media-free project shows nothing
/// and the panel geometry is unchanged.
pub fn piv_media_note(core: &EditorCore) -> Option<String> {
    let segment_media = project_has_segment_media(core);
    let instances = !core.project.sprites.is_empty() || !core.project.texts.is_empty();
    // Wave T (T1): each variant is a catalogue string
    // (IA-T1-1) whose English value is the pre-wire
    // literal, byte for byte.
    match (segment_media, instances) {
        (false, false) => None,
        (true, false) => Some(i18n_wire::tr(core, "export_piv_note_segments")),
        (false, true) => Some(i18n_wire::tr(core, "export_piv_note_instances")),
        (true, true) => Some(i18n_wire::tr(core, "export_piv_note_both")),
    }
}

/// Does the project carry any sprite instances
/// (`Project::sprites`)? Pure: reads the project only.
pub fn project_has_sprites(core: &EditorCore) -> bool {
    !core.project.sprites.is_empty()
}

/// The sprite note (element key `xc-sprite-note`), shown
/// under the figure-file rows when the project has sprite
/// instances a .stk, .nodes or .stknds export would leave
/// behind. Source truth, checked against the writers:
///
/// * `.stk` (`pivot::export_stk`) and `.nodes`
///   (`nodes::export_nodes_figure`) each take one
///   `FigureDef` and write that figure only; sprite
///   instances live on the project, not on a figure, so
///   no instance can appear in either file.
/// * `.stknds` (`nodes_anim::export_nodes_animation`)
///   takes the whole project but writes the sprite
///   library count and every frame's sprite count as
///   zero — its own doc comment lists sprites among the
///   things "written as absent" and "dropped" on
///   reimport — so a whole-project export loses them
///   silently too.
///
/// The .piv formats drop sprite instances as well, but
/// that half is already stated by `piv_media_note` under
/// the .piv rows, so this note does not repeat it. Text
/// instances are likewise outside this note's scope:
/// the .piv note names their .piv drop, and this note
/// exists for the sprite half only. A sprite-free
/// project shows nothing, so its panel is unchanged.
pub fn sprite_note(core: &EditorCore) -> Option<String> {
    if project_has_sprites(core) {
        // Wave T (T1): a catalogue string
        // (`export_sprite_note`, IA-T1-1) whose English
        // value is the pre-wire literal, byte for byte.
        Some(i18n_wire::tr(core, "export_sprite_note"))
    } else {
        None
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
/// makes in "All frames" mode; a bundle job's worker runs
/// the member's hooked entry instead (see
/// `run_bundle_job`), at the same destination.
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

impl BundleKind {
    /// The bundle's name as progress-readout slot data —
    /// the `{name}` the working line interpolates while
    /// a bundle job has no event yet. Like
    /// `ExportOp::name()`, this is data the templates
    /// interpolate, not a label site: it stays English
    /// and is not translated.
    pub fn name(self) -> &'static str {
        match self {
            BundleKind::Images => "images bundle",
            BundleKind::Video => "video bundle",
        }
    }
}

/// Each bundle member's frame total in run order — the
/// allocation the core `BundleProgress` tracker is
/// constructed with, computed before the first member
/// runs (Wave Z, Z8; the allocation rule is X11's):
/// the images members each write one file per authored
/// frame, so each share is the authored frame count;
/// the video members report against the sampled-times
/// count, so each share is
/// `export::sampled_frame_total`. Pure: reads the
/// project only.
pub fn bundle_member_totals(
    kind: BundleKind,
    project: &stykanimate_rs::model::Project,
) -> Vec<usize> {
    match kind {
        BundleKind::Images => vec![project.frames.len(); 4],
        BundleKind::Video => {
            let member = stykanimate_rs::export::sampled_frame_total(project);
            vec![member, member]
        }
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

/// The recap for a failed export, keeping the reason the
/// op put on the status line ("MP4 export failed: ffmpeg
/// exited with …") instead of dropping it to the bare
/// `failed_recap` shape — the recap is the record that
/// persists after the status line and its toast move on,
/// so it understates the failure if it keeps only the
/// format name. The status is used verbatim when it is a
/// failure wording (the `export_failed` convention);
/// anything else falls back to `failed_recap`.
pub fn failure_recap(status: &str, name: &str) -> String {
    if status.to_lowercase().contains("failed") {
        status.to_string()
    } else {
        failed_recap(name)
    }
}

/// The recap line for a successful range export: the
/// frame range is part of the record (the plain
/// destination alone would claim a full export), and a
/// range WebM carries its video-only marker — WebM drops
/// the project's audio in every mode. A range MP4 no
/// longer takes the marker: since Wave 9 it muxes the
/// clips inside its range (any ffmpeg-readable format
/// since Wave 10).
pub fn single_recap_range(name: &str, dest: &str, range: (usize, usize), video_only: bool) -> String {
    let (a, b) = range;
    if video_only {
        format!(
            "Exported {name} (frames {}–{}, video only) → {dest}",
            a + 1,
            b + 1
        )
    } else {
        format!("Exported {name} (frames {}–{}) → {dest}", a + 1, b + 1)
    }
}

/// How much the recap line means it, classified from its
/// own wording by the toast classifier: a recap that
/// names a failure is an Error (a partially failed
/// bundle's recap contains "failed:", so it reads Error
/// too — the same decision `classify_status` already
/// makes for the bundle's toast, pinned by test), and
/// every success recap starts with "Exported ", the
/// classifier's Success shape. The panel render colours
/// the line and its glyph from this severity, so the
/// persistent record carries the same signal the
/// transient toast did.
pub fn recap_severity(recap: &str) -> Severity {
    classify_status(recap)
}

/// Record the outcome of a single-format export in the
/// panel's recap line. `range` is the range the press
/// actually exported (the action's `range_target`, read
/// before the op ran): a range success recaps the frame
/// range (and video-only for WebM) via
/// `single_recap_range` instead of claiming the plain
/// full destination, and a failure keeps the op's reason
/// via `failure_recap`.
fn note_single(s: &mut State, op: ExportOp, range: Option<(usize, usize)>) {
    s.export_recap = if export_failed(&s.core) {
        failure_recap(&s.core.status, op.name())
    } else {
        match range {
            Some(range) => single_recap_range(
                op.name(),
                op.dest(),
                range,
                matches!(op, ExportOp::Webm),
            ),
            None => single_recap(op.name(), op.dest()),
        }
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

/// A bundle button press (Wave Z, Z8): start the
/// bundle's background job — the whole body of the
/// handler now, exactly as the contracted formats'
/// presses are `start_export_job`. The members run on
/// the worker in `plan_bundle` order, always the full
/// exports (bundles ignore the range toggle — see the
/// module docs), and `finish_export_job` has the last
/// word the synchronous handler had: the bundle recap
/// overwrites the status and is stored as the panel
/// recap, so both show the summary rather than
/// whichever member ran last. While any job runs the
/// press is a no-op (one export at a time, W1).
fn run_bundle(s: &mut State, kind: BundleKind) {
    start_bundle_job(s, kind);
}

// ---- Export jobs (Wave W, W1) -----------------------------
// IA-U11-1(b) + IA-V12-1(b), one mechanism for both phase
// contracts. The four contracted formats (PNG/JPEG —
// render phase; MP4/WebM — encode phase, and since W2
// the video funnels' render loop reports too) no longer
// run on the UI thread: a press spawns a worker over a
// cloned `Project`, the contracts' callbacks write ONLY
// into the job's two atomic phase slots, the outcome
// returns over a channel, and `main::poll` drains the
// job through `poll_export_job` on the frame tick. The
// phases' state is kept separate end to end — two slots,
// two readout lines, never a merged figure — and the
// progress text lives only in the readout: `core.status`
// is written exactly once, at completion, with the
// sentences the synchronous paths produced (see
// `finish_export_job`), so the machine-read class-C text
// never carries a progress claim.
//
// Wave Z (Z8) extends the same mechanism to the two
// bundles: a third slot carries the core bundle
// contract's events (Wave X, X11), written ONLY by the
// tracker's callback on the bundle worker. The bundle
// slot is not a third phase and is never merged with
// the phase slots — a bundle job's readout is the one
// bundle line, a single-format job's readout is its
// phase lines, and neither job shape reads the other's
// slots.

/// How long `main::poll` waits before re-waking while an
/// export job is active, so an otherwise idle window
/// keeps repainting the progress readout — the live-MCP
/// heartbeat's cadence.
pub const JOB_WAKE: std::time::Duration = std::time::Duration::from_millis(100);

/// One progress slot — a phase's, or a bundle job's
/// (Wave Z, Z8) — shared between the contract callback
/// (the only writer) and the UI thread (reader). The
/// callback copies each event's
/// fields verbatim — nothing is computed, estimated or
/// interpolated — so every number the panel shows is a
/// number the contract delivered. `seen` separates
/// "this phase has not reported yet" (the readout shows
/// no number for it) from a reported zero.
#[derive(Default)]
pub struct PhaseProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    finished: AtomicBool,
    seen: AtomicBool,
}

impl PhaseProgress {
    fn store_render(&self, ev: stykanimate_rs::export::RenderProgressEvent) {
        self.done.store(ev.rendered_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        // Written last: a reader that sees `seen` also
        // sees this event's numbers.
        self.seen.store(true, Ordering::SeqCst);
    }

    fn store_encode(&self, ev: stykanimate_rs::export::EncodeProgressEvent) {
        self.done.store(ev.encoded_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        self.seen.store(true, Ordering::SeqCst);
    }

    /// The bundle slot's writer (Wave Z, Z8): the core
    /// bundle tracker's callback copies each
    /// `BundleProgressEvent`'s fields verbatim, exactly
    /// as the phase writers copy theirs — the bundle
    /// total was allocated before the first member ran
    /// and the tracker guarantees the counts never run
    /// backwards, so the slot needs no arithmetic of its
    /// own.
    fn store_bundle(&self, ev: stykanimate_rs::export::BundleProgressEvent) {
        self.done.store(ev.completed_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        self.seen.store(true, Ordering::SeqCst);
    }

    /// The slot's current contents, by value.
    pub fn snapshot(&self) -> PhaseSnapshot {
        PhaseSnapshot {
            done: self.done.load(Ordering::SeqCst),
            total: self.total.load(Ordering::SeqCst),
            finished: self.finished.load(Ordering::SeqCst),
            seen: self.seen.load(Ordering::SeqCst),
        }
    }
}

/// A [`PhaseProgress`] slot read by value. `PartialEq`
/// so the tick can tell whether what is on screen
/// changed (and only then ask for a repaint).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PhaseSnapshot {
    pub done: usize,
    pub total: usize,
    pub finished: bool,
    pub seen: bool,
}

/// Which hooked entry a job runs. The range is baked in
/// from the press's `range_target`, so the worker never
/// re-reads shell state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum JobKind {
    PngFull,
    PngRange(usize, usize),
    JpegFull,
    JpegRange(usize, usize),
    Mp4Full,
    Mp4Range(usize, usize),
    WebmFull,
    WebmRange(usize, usize),
    /// The two bundles (Wave Z, Z8): a job whose worker
    /// runs the whole plan, not one format.
    BundleImages,
    BundleVideo,
}

impl JobKind {
    /// The single format a job exports. The bundle
    /// kinds never reach the code that asks this — the
    /// finish path and the readout branch on
    /// `bundle_kind` first — so they map to their first
    /// member purely to keep the match total.
    fn op(self) -> ExportOp {
        match self {
            JobKind::PngFull | JobKind::PngRange(..) => ExportOp::Png,
            JobKind::JpegFull | JobKind::JpegRange(..) => ExportOp::Jpeg,
            JobKind::Mp4Full | JobKind::Mp4Range(..) => ExportOp::Mp4,
            JobKind::WebmFull | JobKind::WebmRange(..) => ExportOp::Webm,
            JobKind::BundleImages => ExportOp::Png,
            JobKind::BundleVideo => ExportOp::Mp4,
        }
    }

    /// The bundle a job runs, if it is a bundle job
    /// (Wave Z, Z8).
    fn bundle_kind(self) -> Option<BundleKind> {
        match self {
            JobKind::BundleImages => Some(BundleKind::Images),
            JobKind::BundleVideo => Some(BundleKind::Video),
            _ => None,
        }
    }

    fn range(self) -> Option<(usize, usize)> {
        match self {
            JobKind::PngRange(a, b)
            | JobKind::JpegRange(a, b)
            | JobKind::Mp4Range(a, b)
            | JobKind::WebmRange(a, b) => Some((a, b)),
            _ => None,
        }
    }
}

/// A finished job's success value, mirroring the hooked
/// entries' own return shapes: a frame count (PNG/JPEG
/// sequences and the full WebM), the MP4 outcome (the
/// frame count plus the missing-audio report), or the
/// range WebM outcome — the same shared outcome type
/// since Wave X (X12) widened the WebM range entries.
#[derive(Debug)]
enum ExportJobResult {
    Frames(usize),
    Mp4(stykanimate_rs::export::Mp4ExportOutcome),
    Webm(stykanimate_rs::export::Mp4ExportOutcome),
    /// A finished bundle (Wave Z, Z8): the members that
    /// failed, in plan order — the exact input
    /// `bundle_recap` took from the synchronous handler.
    /// A bundle job's worker always produces this shape
    /// (a member's failure is data, never the job's
    /// `Err`); the job-level `Err` is reserved for a
    /// worker that produced nothing at all.
    Bundle(Vec<ExportOp>),
}

/// What the worker sends back: the entry's result, with
/// the io error stringified on the worker — the finish
/// path interpolates it into the failure sentence
/// exactly as the synchronous paths interpolated the
/// error itself.
type ExportJobOutcome = Result<ExportJobResult, String>;

/// A running export job, held in `State::export_job`.
/// The worker owns the project clone and the sending
/// end of the outcome channel; everything the UI thread
/// touches is here: the two phase slots, the bundle
/// slot (Wave Z, Z8 — written only by a bundle job's
/// tracker callback; a single-format job never touches
/// it) and the last snapshots it showed (for
/// repaint-on-change).
pub struct ExportJob {
    kind: JobKind,
    dest: PathBuf,
    render: Arc<PhaseProgress>,
    encode: Arc<PhaseProgress>,
    bundle: Arc<PhaseProgress>,
    outcome: mpsc::Receiver<ExportJobOutcome>,
    shown_render: PhaseSnapshot,
    shown_encode: PhaseSnapshot,
    shown_bundle: PhaseSnapshot,
}

/// The unit of work a job's worker runs: the cloned
/// project plus the three progress slots, returning
/// the outcome to send back. Production passes
/// `run_job` bound to the kind and destination (or
/// `run_bundle_job` for a bundle); the pins pass a
/// test worker, so no pin ever writes an export file.
type JobWork = Box<
    dyn FnOnce(
            &stykanimate_rs::model::Project,
            &PhaseProgress,
            &PhaseProgress,
            &PhaseProgress,
        ) -> ExportJobOutcome
        + Send,
>;

/// Is an export job running? While one is, every export
/// press in the panel is a no-op (see `start_export_job`).
pub fn export_job_active(s: &State) -> bool {
    s.export_job.is_some()
}

/// What a press of one of the four contracted formats
/// resolves to: the job kind (the loop range baked in
/// when the toggle applies) and the destination —
/// computed exactly as the synchronous action computed
/// them: `export_dir()` for PNG, MP4 and WebM, the
/// `out/jpeg` directory for JPEG, the file name joined
/// for the video formats. `None` for an op with no
/// progress contract (never jobbed). Pure: no thread,
/// no filesystem.
fn job_spec(s: &State, op: ExportOp) -> Option<(JobKind, PathBuf)> {
    let range = range_target(&s.core, s.export_use_range);
    Some(match (op, range) {
        (ExportOp::Png, Some((a, b))) => (JobKind::PngRange(a, b), export_dir()),
        (ExportOp::Png, None) => (JobKind::PngFull, export_dir()),
        (ExportOp::Jpeg, Some((a, b))) => (
            JobKind::JpegRange(a, b),
            PathBuf::from("out").join("jpeg"),
        ),
        (ExportOp::Jpeg, None) => (JobKind::JpegFull, PathBuf::from("out").join("jpeg")),
        (ExportOp::Mp4, Some((a, b))) => (
            JobKind::Mp4Range(a, b),
            export_dir().join("animation.mp4"),
        ),
        (ExportOp::Mp4, None) => (JobKind::Mp4Full, export_dir().join("animation.mp4")),
        (ExportOp::Webm, Some((a, b))) => (
            JobKind::WebmRange(a, b),
            export_dir().join("animation.webm"),
        ),
        (ExportOp::Webm, None) => {
            (JobKind::WebmFull, export_dir().join("animation.webm"))
        }
        _ => return None,
    })
}

/// The worker body: call the one hooked entry for
/// `kind` over the cloned project, each phase's
/// callback writing only its own slot, and return the
/// entry's result in the job's shape. Runs entirely
/// off the UI thread; touches no shell state.
fn run_job(
    kind: JobKind,
    project: &stykanimate_rs::model::Project,
    dest: &Path,
    render: &PhaseProgress,
    encode: &PhaseProgress,
) -> ExportJobOutcome {
    let render_cb =
        |ev: stykanimate_rs::export::RenderProgressEvent| render.store_render(ev);
    let encode_cb =
        |ev: stykanimate_rs::export::EncodeProgressEvent| encode.store_encode(ev);
    let result = match kind {
        JobKind::PngFull => stykanimate_rs::export::export_png_sequence_with_render_progress(
            project,
            dest,
            Some(&render_cb),
        )
        .map(ExportJobResult::Frames),
        JobKind::PngRange(a, b) => {
            stykanimate_rs::export::export_png_sequence_range_with_render_progress(
                project,
                a,
                b,
                dest,
                Some(&render_cb),
            )
            .map(ExportJobResult::Frames)
        }
        JobKind::JpegFull => {
            stykanimate_rs::export::export_jpeg_sequence_with_render_progress(
                project,
                dest,
                Some(&render_cb),
            )
            .map(ExportJobResult::Frames)
        }
        JobKind::JpegRange(a, b) => {
            stykanimate_rs::export::export_jpeg_sequence_range_with_render_progress(
                project,
                a,
                b,
                dest,
                Some(&render_cb),
            )
            .map(ExportJobResult::Frames)
        }
        JobKind::Mp4Full => stykanimate_rs::export::export_mp4_with_progress(
            project,
            dest,
            Some(&encode_cb),
            Some(&render_cb),
        )
        .map(ExportJobResult::Mp4),
        JobKind::Mp4Range(a, b) => stykanimate_rs::export::export_mp4_range_with_progress(
            project,
            a,
            b,
            dest,
            Some(&encode_cb),
            Some(&render_cb),
        )
        .map(ExportJobResult::Mp4),
        JobKind::WebmFull => stykanimate_rs::export::export_webm_with_progress(
            project,
            dest,
            Some(&encode_cb),
            Some(&render_cb),
        )
        .map(ExportJobResult::Frames),
        JobKind::WebmRange(a, b) => {
            stykanimate_rs::export::export_webm_range_with_progress(
                project,
                a,
                b,
                dest,
                Some(&encode_cb),
                Some(&render_cb),
            )
            .map(ExportJobResult::Webm)
        }
        // Bundle kinds never run here: `begin_job`'s work
        // closure for a bundle calls `run_bundle_job`
        // (the plan-driven worker). Reaching this arm is
        // a dispatch bug, surfaced as a failed job rather
        // than a panic.
        JobKind::BundleImages | JobKind::BundleVideo => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "bundle jobs run through run_bundle_job",
        )),
    };
    result.map_err(|e| e.to_string())
}

/// Store a job in `State` and spawn its worker. The
/// worker owns the project clone, the phase slots and
/// the channel's sending end; it sends exactly one
/// outcome, then exits.
fn begin_job(
    s: &mut State,
    kind: JobKind,
    dest: PathBuf,
    project: stykanimate_rs::model::Project,
    work: JobWork,
) {
    let render = Arc::new(PhaseProgress::default());
    let encode = Arc::new(PhaseProgress::default());
    let bundle = Arc::new(PhaseProgress::default());
    let (tx, rx) = mpsc::channel::<ExportJobOutcome>();
    s.export_job = Some(ExportJob {
        kind,
        dest,
        render: Arc::clone(&render),
        encode: Arc::clone(&encode),
        bundle: Arc::clone(&bundle),
        outcome: rx,
        shown_render: PhaseSnapshot::default(),
        shown_encode: PhaseSnapshot::default(),
        shown_bundle: PhaseSnapshot::default(),
    });
    std::thread::spawn(move || {
        let outcome = work(&project, &render, &encode, &bundle);
        let _ = tx.send(outcome);
    });
}

/// Begin the background job for one of the four
/// contracted formats — the whole body of their button
/// actions now. Returns false, changing nothing, when
/// a job is already active: one export at a time, the
/// contract the old blocking calls enforced by
/// construction (two live exports could write the same
/// destinations — a bundle includes PNG and MP4), with
/// the running readout as the visible reason a press
/// did nothing.
fn start_export_job(s: &mut State, op: ExportOp) -> bool {
    if s.export_job.is_some() {
        return false;
    }
    let Some((kind, dest)) = job_spec(s, op) else {
        return false;
    };
    // The directory creation the MP4/WebM range actions
    // performed before calling (the sequence entries and
    // the encode funnels create their own parents).
    if matches!(kind, JobKind::Mp4Range(..) | JobKind::WebmRange(..)) {
        let _ = std::fs::create_dir_all(export_dir());
    }
    let work_dest = dest.clone();
    let project = s.core.project.clone();
    begin_job(
        s,
        kind,
        dest,
        project,
        Box::new(move |project, render, encode, _bundle| {
            run_job(kind, project, &work_dest, render, encode)
        }),
    );
    true
}

/// What a bundle press resolves to: the job kind for
/// the bundle and the destination root the members
/// build their paths from (`export_dir()` — the JPEG
/// member's `out/jpeg` is the same CWD-relative path
/// the `EditorCore` op uses). Pure: no thread, no
/// filesystem.
fn bundle_job_spec(kind: BundleKind) -> (JobKind, PathBuf) {
    let job_kind = match kind {
        BundleKind::Images => JobKind::BundleImages,
        BundleKind::Video => JobKind::BundleVideo,
    };
    (job_kind, export_dir())
}

/// The bundle worker body (Wave Z, Z8): run the plan's
/// members in order over the cloned project, each
/// through the hooked entry whose plain form the
/// member's `EditorCore` op calls, at the op's own
/// destination, under a core `BundleProgress` tracker
/// allocated from `bundle_member_totals` before the
/// first member runs. The tracker's callback writes
/// ONLY into the job's bundle slot; the phase slots
/// are not touched (the tracker consumes the members'
/// phase events itself). Every member is attempted —
/// a failure is recorded and the bundle continues, as
/// the synchronous handler did — and the tracker is
/// finished after the last member: its terminal event
/// fires only if every member succeeded, the member
/// contracts' own rule one level up. Failure here is
/// the member entries' `Err`, the same signal the
/// `EditorCore` ops turned into their failure statuses
/// (the statuses `export_failed` read); the worker
/// needs no status line of its own. Runs entirely off
/// the UI thread; touches no shell state.
fn run_bundle_job(
    kind: BundleKind,
    project: &stykanimate_rs::model::Project,
    dest: &Path,
    bundle: &PhaseProgress,
) -> ExportJobOutcome {
    use stykanimate_rs::export;
    let bundle_cb = |ev: export::BundleProgressEvent| bundle.store_bundle(ev);
    let tracker = export::BundleProgress::new(
        &bundle_member_totals(kind, project),
        Some(&bundle_cb),
    );
    let mut failed: Vec<ExportOp> = Vec::new();
    match kind {
        BundleKind::Images => {
            let member_cb = tracker.render_callback(0);
            let result = export::export_png_sequence_with_render_progress(
                project,
                dest,
                Some(&member_cb),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Png);
            }
            let member_cb = tracker.render_callback(1);
            let result = export::export_jpeg_sequence_with_render_progress(
                project,
                &PathBuf::from("out").join("jpeg"),
                Some(&member_cb),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Jpeg);
            }
            // SVG and the spritesheet have no phase
            // contract of their own: no translator —
            // their frames bank when they complete
            // (the tracker's documented behaviour).
            let result = stykanimate_rs::svg::export_svg(project, &dest.join("svg"));
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Svg);
            }
            let result = export::export_spritesheet(project, &dest.join("spritesheet.png"));
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Spritesheet);
            }
        }
        BundleKind::Video => {
            let member_cb = tracker.encode_callback(0);
            let result = export::export_mp4_with_encode_progress(
                project,
                &dest.join("animation.mp4"),
                Some(&member_cb),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Mp4);
            }
            let member_cb = tracker.encode_callback(1);
            let result = export::export_webm_with_encode_progress(
                project,
                &dest.join("animation.webm"),
                Some(&member_cb),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Webm);
            }
        }
    }
    tracker.finish();
    Ok(ExportJobResult::Bundle(failed))
}

/// Begin the background job for a bundle — the body of
/// `run_bundle` now. Returns false, changing nothing,
/// when a job is already active: one export at a time,
/// as `start_export_job` (a bundle includes PNG and
/// MP4, so it can no more overlap a running job than
/// another bundle could overlap it).
fn start_bundle_job(s: &mut State, kind: BundleKind) -> bool {
    if s.export_job.is_some() {
        return false;
    }
    let (job_kind, dest) = bundle_job_spec(kind);
    let work_dest = dest.clone();
    let project = s.core.project.clone();
    begin_job(
        s,
        job_kind,
        dest,
        project,
        Box::new(move |project, _render, _encode, bundle| {
            run_bundle_job(kind, project, &work_dest, bundle)
        }),
    );
    true
}

/// The frame-tick drain for the export job, called from
/// `main::poll`. Returns `(redraw, active)`: redraw is
/// asked only when a phase snapshot changed since the
/// last shown one (or the job just finished and its
/// status/recap landed); active is true while the job
/// runs, so the caller can keep the wake cadence
/// (`JOB_WAKE`) alive. A worker that dies without
/// sending (a panic in the export) finishes the job as
/// a failure in the op's voice — never as a success and
/// never silently dropped.
pub fn poll_export_job(s: &mut State) -> (bool, bool) {
    let received = match s.export_job.as_ref() {
        None => return (false, false),
        Some(job) => job.outcome.try_recv(),
    };
    match received {
        Ok(result) => {
            let job = s.export_job.take().expect("job checked present");
            finish_export_job(s, &job, result);
            (true, false)
        }
        Err(mpsc::TryRecvError::Disconnected) => {
            let job = s.export_job.take().expect("job checked present");
            finish_export_job(
                s,
                &job,
                Err("the export worker stopped before reporting a result".to_string()),
            );
            (true, false)
        }
        Err(mpsc::TryRecvError::Empty) => {
            let job = s.export_job.as_mut().expect("job checked present");
            let render = job.render.snapshot();
            let encode = job.encode.snapshot();
            let bundle = job.bundle.snapshot();
            let changed = render != job.shown_render
                || encode != job.shown_encode
                || bundle != job.shown_bundle;
            job.shown_render = render;
            job.shown_encode = encode;
            job.shown_bundle = bundle;
            (changed, true)
        }
    }
}

/// The contracted formats' failure sentence for a
/// reason — the `editor.rs` full ops' and the range
/// actions' voices, which agree per format.
fn job_failure_status(op: ExportOp, reason: &str) -> String {
    match op {
        ExportOp::Png => format!("Export failed: {reason}"),
        ExportOp::Jpeg => format!("JPEG export failed: {reason}"),
        ExportOp::Mp4 => format!("MP4 export failed: {reason}"),
        _ => format!("WebM export failed: {reason}"),
    }
}

/// Turn a finished job's outcome into the panel's
/// status and recap: the exact sentences the
/// synchronous actions (range) and the `EditorCore`
/// full ops produced for the same result — the MP4
/// missing-audio note composed from the outcome in
/// both modes, the range WebM's video-only sentence
/// kept verbatim as the class-C wording of record
/// (its job outcome rides along since Wave X, X12,
/// but the sentence itself is the exclusion's, and
/// does not change) —
/// then `note_single`, as the actions called it.
fn finish_export_job(s: &mut State, job: &ExportJob, result: ExportJobOutcome) {
    // A bundle job (Wave Z, Z8) finishes the way the
    // synchronous bundle handler did: the recap — built
    // from the members that failed — is written to the
    // status AND stored as the panel recap, the summary
    // having the last word over any member's own
    // wording. A job-level `Err` (a worker that died
    // before reporting, or a result shape the worker
    // never pairs with a bundle kind) means no member
    // outcome was reported at all, so every member
    // counts as failed — the recap claims nothing was
    // exported, which is the only honest summary.
    if let Some(kind) = job.kind.bundle_kind() {
        let failed = match result {
            Ok(ExportJobResult::Bundle(failed)) => failed,
            _ => plan_bundle(kind),
        };
        let recap = bundle_recap(kind, &failed);
        s.core.status = recap.clone();
        s.export_recap = recap;
        return;
    }
    let op = job.kind.op();
    s.core.status = match result {
        Err(e) => job_failure_status(op, &e),
        Ok(res) => match (job.kind, res) {
            (JobKind::PngFull, ExportJobResult::Frames(n)) => {
                format!("Exported {n} PNG frames to {}", job.dest.display())
            }
            (JobKind::PngRange(a, b), ExportJobResult::Frames(n)) => format!(
                "Exported {n} PNG frames (frames {}–{}) to {}",
                a + 1,
                b + 1,
                job.dest.display()
            ),
            (JobKind::JpegFull, ExportJobResult::Frames(_)) => {
                "Exported JPEG frames to out/jpeg/.".to_string()
            }
            (JobKind::JpegRange(a, b), ExportJobResult::Frames(n)) => format!(
                "Exported {n} JPEG frames (frames {}–{}) to out/jpeg/.",
                a + 1,
                b + 1
            ),
            (JobKind::Mp4Full, ExportJobResult::Mp4(outcome)) => {
                match outcome.missing_audio_note() {
                    Some(note) => format!(
                        "Exported {}-frame MP4 to {}. {note}",
                        outcome.frames,
                        job.dest.display()
                    ),
                    None => format!(
                        "Exported {}-frame MP4 to {}",
                        outcome.frames,
                        job.dest.display()
                    ),
                }
            }
            (JobKind::Mp4Range(a, b), ExportJobResult::Mp4(outcome)) => {
                match outcome.missing_audio_note() {
                    Some(note) => format!(
                        "Exported MP4 (frames {}–{}) to {}. {note}",
                        a + 1,
                        b + 1,
                        job.dest.display()
                    ),
                    None => format!(
                        "Exported MP4 (frames {}–{}) to {}",
                        a + 1,
                        b + 1,
                        job.dest.display()
                    ),
                }
            }
            (JobKind::WebmFull, ExportJobResult::Frames(n)) => {
                format!("Exported {n}-frame WebM.")
            }
            // The outcome (Wave X, X12) is carried for
            // type parity with the MP4 arms; the sentence
            // is the class-C wording of record and stays
            // byte-identical whether or not the outcome
            // names skipped clips.
            (JobKind::WebmRange(a, b), ExportJobResult::Webm(_)) => format!(
                "Exported WebM (frames {}–{}, video only) to {}",
                a + 1,
                b + 1,
                job.dest.display()
            ),
            // `run_job` pairs each kind with exactly one
            // result shape; any other pairing is a bug in
            // this module, reported as a failure in the
            // op's voice rather than as a success.
            _ => job_failure_status(op, "internal result mismatch"),
        },
    };
    note_single(s, op, job.kind.range());
}

/// The readout's render-phase line: the stored event's
/// own numbers through the catalogue template. The
/// render terminal closes the whole sequence export,
/// so the final event's numbers (`done == total`) are
/// the line until the same tick's outcome replaces the
/// readout with the result — no finished wording is
/// needed, and none is claimed.
fn render_progress_line(core: &EditorCore, snap: PhaseSnapshot) -> String {
    i18n_wire::tr(core, "export_render_progress")
        .replace("{done}", &snap.done.to_string())
        .replace("{total}", &snap.total.to_string())
}

/// The readout's encode-phase line. The encode
/// terminal closes only the video encode — an MP4's
/// audio mux can still run (and fail) after it — so
/// once the stored event is the terminal, the line is
/// the finished sentence and claims exactly that
/// (IA-U11-1(b)): never export success, which exists
/// only when the outcome lands in `finish_export_job`.
fn encode_progress_line(core: &EditorCore, snap: PhaseSnapshot) -> String {
    if snap.finished {
        i18n_wire::tr(core, "export_encode_finished")
    } else {
        i18n_wire::tr(core, "export_encode_progress")
            .replace("{done}", &snap.done.to_string())
            .replace("{total}", &snap.total.to_string())
    }
}

/// The readout's bundle line (Wave Z, Z8): the stored
/// bundle event's own numbers through the catalogue
/// template. The bundle terminal closes the whole
/// bundle, so — like the render line — the final
/// event's numbers (`done == total`) are the line
/// until the same tick's outcome replaces the readout
/// with the recap; no finished wording is needed, and
/// none is claimed. While a member with no contract
/// (SVG, spritesheet) runs, no new event arrives and
/// the line holds at the last true count.
fn bundle_progress_line(core: &EditorCore, snap: PhaseSnapshot) -> String {
    i18n_wire::tr(core, "export_bundle_progress")
        .replace("{done}", &snap.done.to_string())
        .replace("{total}", &snap.total.to_string())
}

/// The progress readout's lines for the active job,
/// each with its element key, built only from the
/// stored snapshots: a seen render phase shows its
/// line, a seen encode phase shows its line, and
/// before either phase has reported, one working line
/// names the format (`ExportOp::name()` is slot data,
/// the recaps' discipline). The two phases are never
/// combined into one figure. A bundle job (Wave Z,
/// Z8) reads its own slot instead: one bundle line
/// from the stored bundle events, or the working line
/// naming the bundle before the first event — the
/// bundle count is a separate core contract, so a
/// bundle job shows no phase line and a phase job no
/// bundle line.
pub fn job_lines(core: &EditorCore, job: &ExportJob) -> Vec<(&'static str, String)> {
    if let Some(kind) = job.kind.bundle_kind() {
        let bundle = job.bundle.snapshot();
        if bundle.seen {
            return vec![("xc-progress-bundle", bundle_progress_line(core, bundle))];
        }
        return vec![(
            "xc-progress-working",
            i18n_wire::tr(core, "export_progress_working").replace("{name}", kind.name()),
        )];
    }
    let mut lines = Vec::new();
    let render = job.render.snapshot();
    if render.seen {
        lines.push(("xc-progress-render", render_progress_line(core, render)));
    }
    let encode = job.encode.snapshot();
    if encode.seen {
        lines.push(("xc-progress-encode", encode_progress_line(core, encode)));
    }
    if lines.is_empty() {
        lines.push((
            "xc-progress-working",
            i18n_wire::tr(core, "export_progress_working")
                .replace("{name}", job.kind.op().name()),
        ));
    }
    lines
}

// ---- Range-capable actions --------------------------------
// Each falls back to the full `EditorCore` op when no range
// target applies, so the button never silently does less
// than its label promises. Destinations and status wording
// mirror the full ops in `editor.rs`.

fn export_png_action(s: &mut State) {
    // Wave W (W1): the press starts the background job
    // (render phase, IA-V12-1) — the hooked entries the
    // job's worker runs are the progress counterparts of
    // the range/full calls that stood here, at the same
    // destinations, and the finish writes their exact
    // status wording (`finish_export_job`).
    start_export_job(s, ExportOp::Png);
}

fn export_jpeg_action(s: &mut State) {
    // Wave W (W1): the background job, as
    // `export_png_action` — the JPEG entries' render
    // phase, the `out/jpeg` destination unchanged.
    start_export_job(s, ExportOp::Jpeg);
}

/// SVG with the range toggle (Phase J): the range variant
/// writes into the same directory the full op uses —
/// `EditorCore::export_svg` writes to `./export/svg` — and
/// reports in the full op's wording with the frame range
/// named.
fn export_svg_action(s: &mut State) {
    // One export at a time (Wave W, W1): while a
    // background job runs, every export press is a
    // no-op — see `start_export_job`.
    if export_job_active(s) {
        return;
    }
    let range = range_target(&s.core, s.export_use_range);
    match range {
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
    note_single(s, ExportOp::Svg, range);
}

fn export_gif_action(s: &mut State) {
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    let range = range_target(&s.core, s.export_use_range);
    match range {
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
    note_single(s, ExportOp::Gif, range);
}

/// Spritesheet with the range toggle (Phase J): the range
/// variant writes the one sheet to the full op's
/// destination (`./export/spritesheet.png`) and reports in
/// the contact-sheet action's voice — frame count, the
/// range, the path.
fn export_sheet_action(s: &mut State) {
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    let range = range_target(&s.core, s.export_use_range);
    match range {
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
    note_single(s, ExportOp::Spritesheet, range);
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
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    let range = range_target(&s.core, s.export_use_range);
    match range {
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
    note_single(s, ExportOp::ContactSheet, range);
}

fn export_apng_action(s: &mut State) {
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    let range = range_target(&s.core, s.export_use_range);
    match range {
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
    note_single(s, ExportOp::Apng, range);
}

fn export_mp4_action(s: &mut State) {
    // Wave W (W1): the press starts the background job
    // (encode phase, IA-U11-1; the funnel's render loop
    // reports too since W2). The range behaviour the
    // synchronous call had stands at the entry the
    // worker runs: since Wave 9 the range path muxes
    // the audio of clips intersecting the range, like
    // the full op (see `export::export_mp4_range` docs);
    // since Wave 10 that covers any ffmpeg-readable
    // format, and a clip whose file cannot be probed or
    // decoded fails the export instead of being dropped
    // silently. The Wave P (P15) missing-audio naming
    // happens in `finish_export_job`, from the outcome
    // the worker returns.
    start_export_job(s, ExportOp::Mp4);
}

fn export_webm_action(s: &mut State) {
    // Wave W (W1): the background job, as
    // `export_mp4_action`. The finish reproduces the
    // synchronous wording verbatim — including the
    // range success's video-only sentence, the class-C
    // wording of record (W6 flagged its staleness
    // against the range audio mux rather than editing
    // around the exclusion; the correction is a
    // catalogue-side decision, not this slice's).
    start_export_job(s, ExportOp::Webm);
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
/// scan reads these two directories plus the sequence
/// subdirectories inside them (see `scan_triples`).
pub fn history_roots() -> Vec<PathBuf> {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![base.join("export"), base.join("out")]
}

/// One row of the Recent exports list: something an
/// export wrote, with its size in bytes and its
/// modification time. The display name keeps the output
/// root as a prefix ("export/animation.mp4",
/// "out/animation.apng") because the two roots are both
/// in play and a bare file name does not say which one
/// the file came from. `count` is how many files the row
/// stands for: 1 for a single file, N for a collapsed
/// sequence (a root's `frame-*.png` files, or one of the
/// sequence directories) — the row line names the count
/// when it is more than one, and `size` is then the
/// files' total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportRow {
    pub name: String,
    pub size: u64,
    pub mtime: SystemTime,
    pub count: usize,
}

/// One raw scan result — (path, size in bytes, mtime,
/// file count) — the plain-data input the row model is
/// built from, so the model builders stay pure and
/// testable. Plain files carry a count of 1; the scan's
/// sequence-directory entries carry the directory's file
/// count and total size (see `scan_sequence_dir`).
pub type FileTriple = (PathBuf, u64, SystemTime, usize);

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

/// Is this path one of a sequence export's frame files —
/// `frame-0001.png`, `frame-0012.jpg`, …? The sequence
/// writers in `export.rs` name every frame
/// `frame-{:04}` plus the format extension, so the test
/// is the `frame-` prefix followed by a digit (a
/// hand-named "frame-draft.png" in the same directory is
/// not a sequence frame and keeps its own row).
pub fn is_sequence_frame(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    match name.strip_prefix("frame-") {
        Some(rest) => rest.chars().next().is_some_and(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Build the Recent exports rows from raw scan triples:
/// newest first (name ascending breaks mtime ties, so the
/// order is deterministic), capped at `HISTORY_LIMIT`.
///
/// Sequence frames written into the same directory
/// collapse into ONE row first — a 30-frame PNG export
/// writes 30 `frame-*.png` files straight into
/// `./export/`, and listing them individually would fill
/// every row of the history and push each meaningful
/// export out of it. The collapsed row is named
/// `frame-*.png` in that directory, carries the frames'
/// total size, newest mtime and file count. A directory
/// holding a single frame file is not a flood, so that
/// file keeps a row under its own name.
pub fn rows_from_triples(triples: &[FileTriple]) -> Vec<ExportRow> {
    let mut rows: Vec<ExportRow> = Vec::new();
    // Sequence-frame groups, keyed by parent directory:
    // (dir, total size, newest mtime, file count, the one
    // frame's own path while the group has a single file).
    let mut groups: Vec<(PathBuf, u64, SystemTime, usize, PathBuf)> = Vec::new();
    for (path, size, mtime, count) in triples {
        if is_sequence_frame(path) {
            let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            match groups.iter_mut().find(|(d, ..)| *d == dir) {
                Some((_, total, newest, n, _)) => {
                    *total += size;
                    *newest = (*newest).max(*mtime);
                    *n += count;
                }
                None => groups.push((dir, *size, *mtime, *count, path.clone())),
            }
        } else {
            rows.push(ExportRow {
                name: display_name(path),
                size: *size,
                mtime: *mtime,
                count: *count,
            });
        }
    }
    for (dir, total, newest, n, single) in groups {
        if n > 1 {
            rows.push(ExportRow {
                name: display_name(&dir.join("frame-*.png")),
                size: total,
                mtime: newest,
                count: n,
            });
        } else {
            rows.push(ExportRow {
                name: display_name(&single),
                size: total,
                mtime: newest,
                count: 1,
            });
        }
    }
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
/// Wave T (T1): the buckets render through the
/// catalogue (`export_age_*`, IA-T1-1 — the counted
/// buckets are `{count}` templates), so the function
/// takes the core the way `follow_label` does; English
/// output is byte-identical to the pre-wire literals.
pub fn age_label(core: &EditorCore, now: SystemTime, mtime: SystemTime) -> String {
    let secs = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);
    if secs < 60 {
        i18n_wire::tr(core, "export_age_now")
    } else if secs < 60 * 60 {
        i18n_wire::tr(core, "export_age_min").replace("{count}", &(secs / 60).to_string())
    } else if secs < 24 * 60 * 60 {
        i18n_wire::tr(core, "export_age_hour")
            .replace("{count}", &(secs / (60 * 60)).to_string())
    } else {
        i18n_wire::tr(core, "export_age_day")
            .replace("{count}", &(secs / (24 * 60 * 60)).to_string())
    }
}

/// One Recent exports line: name, size, age — with the
/// file count named after the name for a collapsed
/// sequence row ("export/frame-*.png · 30 files · …"),
/// so the row says it stands for a sequence, not a file
/// literally named with an asterisk. Wave T (T1): the
/// two shapes are catalogue templates
/// (`export_history_row*`, IA-T1-1) with `{name}` /
/// `{count}` / `{size}` / `{age}` slots — the name,
/// size and age ride as slot data (a path, a byte
/// count, the `age_label` line), the frame words are
/// the template's; English output is byte-identical.
pub fn row_line(core: &EditorCore, row: &ExportRow, now: SystemTime) -> String {
    if row.count > 1 {
        i18n_wire::tr(core, "export_history_row_files")
            .replace("{name}", &row.name)
            .replace("{count}", &row.count.to_string())
            .replace("{size}", &format_size(row.size))
            .replace("{age}", &age_label(core, now, row.mtime))
    } else {
        i18n_wire::tr(core, "export_history_row")
            .replace("{name}", &row.name)
            .replace("{size}", &format_size(row.size))
            .replace("{age}", &age_label(core, now, row.mtime))
    }
}

/// The subdirectory names the sequence exports write
/// into, relative to an output root: SVG frames land in
/// `export/svg/`, JPEG frames in `out/jpeg/`. The history
/// represents each as ONE row (see `scan_sequence_dir`)
/// instead of entering it per frame — or ignoring it, as
/// the scan did before, which made JPEG and SVG exports
/// invisible in the history entirely.
pub const SEQUENCE_SUBDIRS: [&str; 2] = ["svg", "jpeg"];

/// The thin filesystem half of the history: read the top
/// level of each root and collect a `FileTriple` for every
/// regular file in it, then collapse each root's sequence
/// subdirectories (`SEQUENCE_SUBDIRS`) into one triple
/// apiece. Missing/unreadable roots and entries whose
/// metadata fails are skipped — an absent export
/// directory is the normal "nothing exported yet" state,
/// not an error. Other subdirectories are not entered.
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
            out.push((entry.path(), md.len(), mtime, 1));
        }
        for sub in SEQUENCE_SUBDIRS {
            scan_sequence_dir(&root.join(sub), &mut out);
        }
    }
    out
}

/// Collapse one sequence directory (`export/svg/`,
/// `out/jpeg/`) into a single triple: the directory's own
/// path (the row model names it "export/svg" /
/// "out/jpeg"), the total size of the regular files
/// directly inside it, the DIRECTORY's mtime — writing a
/// frame into it is what bumps it, so it is the
/// sequence's honest "last exported" time, with the
/// newest file's mtime as fallback — and the file count.
/// An absent directory, or one holding no regular files,
/// contributes nothing: no row for an export that never
/// happened.
fn scan_sequence_dir(dir: &Path, out: &mut Vec<FileTriple>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut total: u64 = 0;
    let mut count: usize = 0;
    let mut newest: Option<SystemTime> = None;
    for entry in entries.filter_map(|e| e.ok()) {
        let Ok(md) = entry.metadata() else {
            continue;
        };
        if !md.is_file() {
            continue;
        }
        total += md.len();
        count += 1;
        if let Ok(mtime) = md.modified() {
            newest = Some(newest.map_or(mtime, |n: SystemTime| n.max(mtime)));
        }
    }
    if count == 0 {
        return;
    }
    if let Some(mtime) = dir_mtime(dir).or(newest) {
        out.push((dir.to_path_buf(), total, mtime, count));
    }
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
/// building the panel costs a few `stat`s instead of two
/// directory scans, and any export that creates or deletes
/// a file in a root bumps that root's mtime, so the next
/// build rescans. The cache key also carries each root's
/// sequence-subdirectory mtimes (`export/svg/`,
/// `out/jpeg/`): writing a frame into one of those bumps
/// the subdirectory's mtime but not its root's, and
/// without it in the key a JPEG/SVG re-export would leave
/// the collapsed row stale. The honest limit, the same
/// class as the preset cache's documented contract:
/// overwriting an existing file in place (re-exporting
/// the same format to the same name) changes no
/// directory's mtime, so that row's size/age can be one
/// scan stale until some create/delete lands in a
/// directory the key watches. When no directory in the
/// key can be statted (no exports yet, usually) the cache
/// is bypassed and the scan runs live — it returns empty,
/// which the panel renders as "No exports yet".
pub fn recent_exports_cached() -> Vec<ExportRow> {
    let roots = history_roots();
    let keys: Vec<Option<SystemTime>> = roots
        .iter()
        .flat_map(|d| {
            let mut v = vec![dir_mtime(d)];
            for sub in SEQUENCE_SUBDIRS {
                v.push(dir_mtime(&d.join(sub)));
            }
            v
        })
        .collect();
    if keys.iter().all(|k| k.is_none()) {
        return rows_from_triples(&scan_triples(&roots));
    }
    HISTORY_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        cached_history(&mut slot, keys, || rows_from_triples(&scan_triples(&roots)))
    })
}

// ---- Rows and groups -----------------------------------
// The category groups (Wave 15 grouping: Images /
// Animation / Video / Project formats) are plain columns
// built inline in `export_panel`, each headed by the
// kit's `design::section_label` — the Phase N form of
// the private section helpers this file used to keep.
// The icon variant's heading stays a local row: a 12 pt
// `icon_el` glyph in the secondary colour before the
// kit label, the inline-glyph idiom frame_ops and
// fx_browser use (Phase H).

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
    label: &str,
    note: &str,
    t: &Theme,
    op: fn(&mut EditorCore),
    recap_name: &'static str,
    recap_dest: &'static str,
) -> El {
    let handler = move |s: &mut State| {
        // One export at a time (Wave W, W1): while a
        // background job runs, a press here is a no-op.
        if export_job_active(s) {
            return;
        }
        op(&mut s.core);
        s.export_recap = if export_failed(&s.core) {
            failure_recap(&s.core.status, recap_name)
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
        .child(
            text(note)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .grow(1.0),
        )
}

/// One format row for a range-capable format: the button
/// (an `icon_btn` with the format type's glyph before the
/// label — Phase H; same key, same label, same handler)
/// is one of the `export_*_action` functions above, which
/// choose between the range exporter and the full op at
/// press time. The label is a borrowed `&str`, not
/// `&'static str`, so a call site can pass the
/// `i18n_wire::tr` translation of a catalogue key (Wave R,
/// R5) — the button builder copies it into the element
/// inside this call, like the chrome_view call sites that
/// pass `&i18n_wire::tr(...)` straight to `icon_btn`.
fn range_row(
    key: &'static str,
    icon: &str,
    label: &str,
    note: &str,
    t: &Theme,
    action: fn(&mut State),
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(icon_btn(key, icon, label, false, t, action))
        .child(
            text(note)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .grow(1.0),
        )
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
    label: &str,
    note: &str,
    t: &Theme,
    kind: BundleKind,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(icon_btn(key, icon, label, false, t, move |s: &mut State| {
            run_bundle(s, kind)
        }))
        .child(
            text(note)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .grow(1.0),
        )
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
    let mut panel = design::panel_col("export-panel", t);
    // Card header (charter §2): the kit's `panel_title`
    // over a hairline divider. The header row has never
    // carried a key and invents none now: it is a plain
    // row wearing the header layout (full width, gap 8,
    // centre-aligned), like the settings card's. The
    // subtitle stays as the first body line under the
    // divider — it is descriptive copy, not a count, so
    // it does not move into the title's meta slot
    // (P04 §3).
    panel = panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(design::panel_title(
                None,
                &i18n_wire::tr(core, "ws_export"),
                None,
                t,
            )),
    );
    panel = panel.child(design::divider(t));
    panel = panel.child(
        text(i18n_wire::tr(core, "export_subtitle"))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    if let Some(note) = loop_note(core) {
        panel = panel.child(
            text(note)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    let toggle = row()
        .key("xc-range-toggle")
        .gap(8.0)
        .center_y()
        .child(
            text(format!("{}:", i18n_wire::tr(core, "frames")))
                .font(type_::LABEL)
                .color(t.text),
        )
        .child(btn("xc-range-all", &i18n_wire::tr(core, "export_all_frames"), !use_range, t, |s| {
            s.export_use_range = false;
        }))
        .child(btn("xc-range-loop", "Loop range", use_range, t, |s| {
            s.export_use_range = true;
        }));
    panel = panel.child(toggle);
    if range_audio_note_shown(core, use_range) {
        panel = panel.child(
            text(i18n_wire::tr(core, "export_range_audio_note"))
                .key("xc-range-audio-note")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    panel = panel.child(
        text(effective_text(core, use_range))
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
    );
    panel = panel.child(
        text(output_text(core, use_range))
            .key("xc-output")
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    let images = col()
        .gap(6.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el("image", 12.0, t.text_secondary))
                .child(design::section_label(&i18n_wire::tr(core, "export_sec_images"), t)),
        )
        .child(range_row(
            "xc-png",
            "image",
            "PNG",
            &i18n_wire::tr(core, "export_note_png"),
            t,
            export_png_action,
        ))
        .child(range_row(
            "xc-jpeg",
            "image",
            &i18n_wire::tr(core, "export_jpeg"),
            &i18n_wire::tr(core, "export_note_jpeg"),
            t,
            export_jpeg_action,
        ))
        .child(range_row(
            "xc-svg",
            "image",
            &i18n_wire::tr(core, "export_svg"),
            &i18n_wire::tr(core, "export_note_svg"),
            t,
            export_svg_action,
        ))
        .child(range_row(
            "xc-sheet",
            "table-cells-large",
            &i18n_wire::tr(core, "spritesheet"),
            &i18n_wire::tr(core, "export_note_sheet"),
            t,
            export_sheet_action,
        ))
        .child(range_row(
            "xc-contact",
            "table-cells-large",
            &i18n_wire::tr(core, "export_contact_sheet"),
            &i18n_wire::tr(core, "export_note_contact"),
            t,
            export_contact_action,
        ));
    panel = panel.child(images);

    let animation = col()
        .gap(6.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el("film", 12.0, t.text_secondary))
                .child(design::section_label(&i18n_wire::tr(core, "export_sec_animation"), t)),
        )
        .child(range_row(
            "xc-gif",
            "film",
            "GIF",
            &i18n_wire::tr(core, "export_note_gif"),
            t,
            export_gif_action,
        ))
        .child(range_row(
            "xc-apng",
            "film",
            &i18n_wire::tr(core, "export_apng"),
            &i18n_wire::tr(core, "export_note_apng"),
            t,
            export_apng_action,
        ));
    panel = panel.child(animation);

    let video = col()
        .gap(6.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el("video", 12.0, t.text_secondary))
                .child(design::section_label(&i18n_wire::tr(core, "export_sec_video"), t)),
        )
        .child(range_row(
            "xc-mp4",
            "video",
            "MP4",
            &i18n_wire::tr(core, "export_note_mp4"),
            t,
            export_mp4_action,
        ))
        .child(range_row(
            "xc-webm",
            "video",
            &i18n_wire::tr(core, "export_webm"),
            &i18n_wire::tr(core, "export_note_webm"),
            t,
            export_webm_action,
        ));
    panel = panel.child(video);

    let mut formats = col()
        .gap(6.0)
        .child(design::section_label(&i18n_wire::tr(core, "export_sec_project_formats"), t))
        .child(plain_row(
            "xc-v2",
            None,
            ".piv v2",
            &i18n_wire::tr(core, "export_note_piv2"),
            t,
            EditorCore::export_piv2,
            ".piv v2",
            "./export/animation-v2.piv",
        ))
        .child(plain_row(
            "xc-v3",
            None,
            ".piv v3",
            &i18n_wire::tr(core, "export_note_piv3"),
            t,
            EditorCore::export_piv,
            ".piv v3",
            "./export/animation.piv",
        ))
        .child(plain_row(
            "xc-v4",
            None,
            ".piv v4",
            &i18n_wire::tr(core, "export_note_piv4"),
            t,
            EditorCore::export_piv4,
            ".piv v4",
            "./export/animation-v4.piv",
        ))
        .child(plain_row(
            "xc-v5",
            None,
            ".piv v5",
            &i18n_wire::tr(core, "export_note_piv5"),
            t,
            EditorCore::export_v5_file,
            ".piv v5",
            "out/animation-v5.piv",
        ));
    // The .piv media note sits directly under the four
    // .piv rows it describes (and above the figure-file
    // rows it does not): a wrapped line inside the card,
    // the range audio note's treatment, present only when
    // the project carries media a .piv export would drop.
    if let Some(note) = piv_media_note(core) {
        formats = formats.child(
            text(note)
                .key("xc-piv-media-note")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    formats = formats
        .child(plain_row(
            "xc-stk",
            None,
            ".stk",
            &i18n_wire::tr(core, "export_note_stk"),
            t,
            EditorCore::export_stk,
            ".stk",
            "./export/<figure>.stk",
        ))
        .child(plain_row(
            "xc-nodes",
            None,
            &i18n_wire::tr(core, "export_nodes"),
            &i18n_wire::tr(core, "export_note_nodes"),
            t,
            EditorCore::export_nodes,
            ".nodes",
            "./export/<figure>.nodes",
        ))
        .child(plain_row(
            "xc-stknds",
            None,
            &i18n_wire::tr(core, "export_stknds"),
            &i18n_wire::tr(core, "export_note_stknds"),
            t,
            EditorCore::export_stknds,
            ".stknds",
            "./export/animation.stknds",
        ));
    // The sprite note sits directly under the three
    // figure-file rows it describes: a wrapped line
    // inside the card, the .piv media note's treatment,
    // present only when the project has sprite instances
    // those exports would leave behind.
    if let Some(note) = sprite_note(core) {
        formats = formats.child(
            text(note)
                .key("xc-sprite-note")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    panel = panel.child(formats);

    let bundles = col()
        .gap(6.0)
        .child(design::section_label(&i18n_wire::tr(core, "export_sec_bundles"), t))
        .child(bundle_row(
            "xc-bundle-images",
            "image",
            &i18n_wire::tr(core, "export_bundle_images"),
            &i18n_wire::tr(core, "export_note_bundle_images"),
            t,
            BundleKind::Images,
        ))
        .child(bundle_row(
            "xc-bundle-video",
            "video",
            &i18n_wire::tr(core, "export_bundle_video"),
            &i18n_wire::tr(core, "export_note_bundle_video"),
            t,
            BundleKind::Video,
        ));
    panel = panel.child(bundles);

    if !s.export_recap.is_empty() {
        // The recap is the panel's persistent record of the
        // last export, so it carries the severity the toast
        // showed transiently: the toast palette's glyph at
        // the leading edge and its colour on the text
        // (success reads green, any failure — including a
        // partially failed bundle — reads error red; see
        // `recap_severity`). The row key is unchanged.
        let sev = recap_severity(&s.export_recap);
        panel = panel.child(
            row()
                .key("xc-recap")
                .gap(8.0)
                .center_y()
                .child(icon_el_rgb(sev.icon(), 12.0, sev.icon_color(t)))
                .child(
                    text(s.export_recap.clone())
                        .font(type_::LABEL)
                        .color(sev.color(t))
                        .wrap()
                        .grow(1.0),
                ),
        );
    }

    if let Some(job) = &s.export_job {
        // The running export job's progress readout
        // (Wave W, W1): one line per phase that has
        // reported, built only from the stored contract
        // events (`job_lines`) — a phase with no event
        // yet shows no number, and the two phases are
        // never combined into one figure. A bundle job
        // (Wave Z, Z8) shows its one bundle line from
        // the bundle slot instead. The block exists
        // only while a job runs, so an idle panel's
        // tree is unchanged.
        let mut progress = col().key("xc-progress").gap(2.0);
        for (key, line) in job_lines(core, job) {
            progress = progress.child(
                text(line)
                    .key(key)
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );
        }
        panel = panel.child(progress);
    }

    // Recent exports: what is actually on disk in the two
    // output roots, newest first — read-only rows, no
    // open/reveal buttons (the shell has no file opener).
    let mut recent = col()
        .gap(6.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el("clock", 12.0, t.text_secondary))
                .child(design::section_label(&i18n_wire::tr(core, "export_sec_recent"), t)),
        )
        .key("xc-history");
    let history = recent_exports_cached();
    if history.is_empty() {
        recent = recent.child(design::empty_state(
            "clock",
            &i18n_wire::tr(core, "export_no_exports_yet"),
            t,
        ));
    } else {
        let now = SystemTime::now();
        for r in &history {
            recent = recent.child(
                text(row_line(core, r, now))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );
        }
    }
    panel = panel.child(recent);

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- i18n wire (Wave R, R5) ----

    #[test]
    fn wired_labels_exist_in_the_catalogue_and_match_english_byte_identically() {
        // Every label `export_panel` now sources from the
        // catalogue must (a) exist in the table — a missing
        // key renders the key text itself — and (b) resolve
        // in English to exactly the string the panel showed
        // before the wire, so English rendering is
        // byte-identical by construction.
        let core = EditorCore::new();
        assert_eq!(core.lang, stykanimate_rs::i18n::Lang::En, "default core is English");
        for (key, literal) in [
            ("ws_export", "Export"),
            ("frames", "Frames"),
            ("export_svg", "SVG"),
            ("spritesheet", "Spritesheet"),
            ("export_webm", "WebM"),
            // Wave S (S1): the block added for R5's filed
            // gaps — each new key's English value is the
            // pre-wire literal at its call site.
            ("export_sec_images", "Images"),
            ("export_sec_animation", "Animation"),
            ("export_sec_video", "Video"),
            ("export_sec_project_formats", "Project formats"),
            ("export_sec_bundles", "Bundles"),
            ("export_sec_recent", "Recent exports"),
            ("export_jpeg", "JPEG"),
            ("export_apng", "APNG"),
            ("export_contact_sheet", "Contact sheet"),
            ("export_nodes", ".nodes"),
            ("export_stknds", ".stknds"),
            ("export_all_frames", "All frames"),
            ("export_bundle_images", "Export all images"),
            ("export_bundle_video", "Export all video"),
            ("export_no_exports_yet", "No exports yet"),
            ("export_subtitle", "Export centre — every format in one place"),
            // Wave T (T1, IA-T1-1): the class-C plain
            // strings — every row guidance note, the
            // static prose lines, the age bucket word.
            // This half of the pin passes only once the
            // integrator inserts the keys (the Q7
            // pattern): until then `tr` falls back to
            // the key text and `key_in_table` is false.
            ("export_note_png", "PNG image sequence — one file per frame in ./export/"),
            ("export_note_jpeg", "JPEG image sequence in out/jpeg/ (out/, not export/)"),
            ("export_note_svg", "One SVG per frame in ./export/svg/"),
            ("export_note_sheet", "Frames on one sheet — ./export/spritesheet.png"),
            ("export_note_contact", "Frames as one tile sheet — ./export/contact-sheet.png"),
            ("export_note_gif", "Animated GIF — ./export/animation.gif"),
            ("export_note_apng", "Animated PNG — out/animation.apng (out/, not export/)"),
            ("export_note_mp4", "MP4 video via ffmpeg — ./export/animation.mp4 (full export includes audio; range export includes audio from clips inside the range — WAV or any other ffmpeg-readable format; a clip file that cannot be probed or decoded fails the range export)"),
            ("export_note_webm", "WebM video (VP9) — ./export/animation.webm (full export is video-only; range export includes audio from clips inside the range)"),
            ("export_note_piv2", "Pivot v2 project — ./export/animation-v2.piv"),
            ("export_note_piv3", "Pivot v3 project — ./export/animation.piv"),
            ("export_note_piv4", "Pivot v4 project — ./export/animation-v4.piv"),
            ("export_note_piv5", "Pivot v5 project — out/animation-v5.piv (out/, not export/)"),
            ("export_note_stk", "Selected figure only — ./export/<figure>.stk"),
            ("export_note_nodes", "Selected figure for Stick Nodes — ./export/<figure>.nodes"),
            ("export_note_stknds", "Whole project for Stick Nodes — ./export/animation.stknds"),
            ("export_note_bundle_images", "PNG + JPEG + SVG + spritesheet in one press. Bundles always export in full — the range toggle above applies only to the single-format buttons."),
            ("export_note_bundle_video", "MP4 + WebM in one press, full exports (the MP4 keeps its audio). Bundles always export in full — the range toggle above applies only to the single-format buttons."),
            ("export_effective_none", "The project has no frames to export."),
            ("export_effective_no_range", "Loop range selected, but no loop range is set — exports write all frames. Set a loop range in the timeline first."),
            ("export_range_audio_note", "Range MP4 and WebM exports include audio from WAV and other ffmpeg-readable clips cut to the range — a clip whose file cannot be probed or decoded fails the export instead of being left out silently."),
            ("export_piv_note_segments", "Media note: .piv v2/v3/v4 export image and text segments as plain segments — only v5 keeps segment media. Not even it keeps a media's local payload: the transform and opacity stay behind."),
            ("export_piv_note_instances", "Media note: no .piv version writes sprite or text instances — they are left out of .piv exports."),
            ("export_piv_note_both", "Media note: .piv v2/v3/v4 export image and text segments as plain segments (only v5 keeps them), and no .piv version writes sprite or text instances — they are left out of .piv exports. Neither keeps a media's local payload: the transform and opacity stay behind."),
            ("export_sprite_note", "Sprite note: .stk and .nodes carry only the selected figure, so project sprite instances are left out of those two exports. .stknds carries the sprite tracks (on re-import, groups come back as plain images and text instances are not carried)."),
            ("export_age_now", "just now"),
        ] {
            assert!(
                crate::rune_sec::i18n_wire::key_in_table(key),
                "{key} must exist in the i18n table"
            );
            assert_eq!(
                crate::rune_sec::i18n_wire::tr(&core, key),
                literal,
                "{key} must render byte-identically in English"
            );
        }
        // The toggle caption composes its colon onto the
        // translated word; in English that composition is
        // the pre-wire literal.
        assert_eq!(
            format!("{}:", crate::rune_sec::i18n_wire::tr(&core, "frames")),
            "Frames:"
        );
    }

    #[test]
    fn class_c_templates_exist_and_compose_byte_identically() {
        // Wave T (T1, IA-T1-1): the class-C template
        // keys — the English value of each is the
        // template itself, slots included, and the
        // composing helpers replace the slots with the
        // same values the pre-wire `format!` calls
        // interpolated, so the composed English lines
        // are byte-identical. Like the plain half of
        // the pin above, this passes only once the
        // integrator inserts the keys (the Q7 pattern).
        let mut core = EditorCore::new();
        for (key, template) in [
            ("export_loop_note", "Loop range: frames {from}–{to} — with \"Loop range\" selected below, the PNG/JPEG/SVG/spritesheet/contact sheet/GIF/APNG/MP4/WebM exports write only those frames (range MP4 and WebM exports include audio from WAV and other ffmpeg-readable clips inside the range — a clip whose file cannot be probed or decoded fails the export)."),
            ("export_effective_range", "Range-capable exports write frames {from}–{to} (the loop range); other formats export in full."),
            ("export_effective_all", "Exports write all {count} frames."),
            ("export_output", "Output: {frames} frames · {width}×{height} · {fps} fps"),
            ("export_age_min", "{count} min ago"),
            ("export_age_hour", "{count} h ago"),
            ("export_age_day", "{count} d ago"),
            ("export_history_row", "{name} · {size} · {age}"),
            ("export_history_row_files", "{name} · {count} files · {size} · {age}"),
        ] {
            assert!(
                crate::rune_sec::i18n_wire::key_in_table(key),
                "{key} must exist in the i18n table"
            );
            assert_eq!(
                crate::rune_sec::i18n_wire::tr(&core, key),
                template,
                "{key} must carry its slots in English"
            );
        }
        // Composed spot checks through the public
        // helpers: the loop note with a set range, and
        // a counted history row — the exact pre-wire
        // sentences.
        core.loop_range = Some((1, 2));
        assert_eq!(
            loop_note(&core).expect("note with a loop range set"),
            "Loop range: frames 2–3 — with \"Loop range\" selected below, the PNG/JPEG/SVG/spritesheet/contact sheet/GIF/APNG/MP4/WebM exports write only those frames (range MP4 and WebM exports include audio from WAV and other ffmpeg-readable clips inside the range — a clip whose file cannot be probed or decoded fails the export)."
        );
        let row = ExportRow {
            name: "export/frame-*.png".into(),
            size: 600,
            mtime: at(300),
            count: 3,
        };
        assert_eq!(
            row_line(&core, &row, at(300)),
            "export/frame-*.png · 3 files · 600 B · just now"
        );
    }

    #[test]
    fn wired_labels_follow_the_core_language() {
        // The wire is live, not a fallback coincidence: in
        // Bengali the translated keys resolve from the real
        // BN table (values read from src/i18n.rs), while
        // the format-name keys the BN table carries in
        // English stay English.
        let mut core = EditorCore::new();
        core.lang = stykanimate_rs::i18n::Lang::Bn;
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "ws_export"), "এক্সপোর্ট");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "frames"), "ফ্রেমসমূহ");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "spritesheet"), "স্প্রাইটশিট");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_svg"), "SVG");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_webm"), "WebM");
        // Wave S (S1) keys: the BN table carries genuine
        // translations for the new block (values read from
        // src/i18n.rs); the format names and extensions
        // stay in Latin script there too.
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_sec_images"), "ছবি");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_all_frames"), "সব ফ্রেম");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_no_exports_yet"), "এখনও কোনো এক্সপোর্ট নেই");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_jpeg"), "JPEG");
        assert_eq!(crate::rune_sec::i18n_wire::tr(&core, "export_nodes"), ".nodes");
    }

    #[test]
    fn export_panel_constructs_under_both_themes_and_languages() {
        // Construction is the assertable level (element
        // internals are opaque — the frame_ops precedent):
        // the wired panel builds with an English core and
        // with a Bengali core, under both themes, with the
        // range branches taken as well as not.
        let mut s = crate::init();
        let _ = export_panel(&s, &rune::tok::theme(true));
        let _ = export_panel(&s, &rune::tok::theme(false));
        s.core.lang = stykanimate_rs::i18n::Lang::Bn;
        let _ = export_panel(&s, &rune::tok::theme(true));
        s.core.loop_range = Some((0, 0));
        s.export_use_range = true;
        let _ = export_panel(&s, &rune::tok::theme(false));
    }

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
        // Wave W (W6): range WebM exports carry audio too
        // (Opus), so the note no longer calls the video
        // formats video-only — the caveat now names both
        // video formats in the range list, and the audio
        // clause below states what they carry.
        assert!(
            note.contains("MP4/WebM exports write only those frames"),
            "video caveat missing: {note}"
        );
        // Wave 11 (C9): the audio clause must state the
        // Wave 10 truth, not the Wave 9 WAV-only version —
        // non-WAV clips sound in a range MP4, and an
        // unprobeable clip file fails the export.
        assert!(
            note.contains("WAV and other ffmpeg-readable clips"),
            "range audio formats misstated: {note}"
        );
        assert!(
            note.contains("cannot be probed or decoded fails the export"),
            "unprobeable-clip failure missing: {note}"
        );
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
    fn range_recap_names_the_frames_and_video_only() {
        assert_eq!(
            single_recap_range("PNG", "./export/", (1, 2), false),
            "Exported PNG (frames 2–3) → ./export/"
        );
        assert_eq!(
            single_recap_range("WebM", "./export/animation.webm", (0, 4), true),
            "Exported WebM (frames 1–5, video only) → ./export/animation.webm"
        );
        // A range MP4 no longer takes the marker (Wave 9:
        // it muxes the clips inside its range — any
        // ffmpeg-readable format since Wave 10).
        assert_eq!(
            single_recap_range("MP4", "./export/animation.mp4", (0, 4), false),
            "Exported MP4 (frames 1–5) → ./export/animation.mp4"
        );
    }

    #[test]
    fn failure_recap_keeps_the_ops_reason() {
        // The status the op wrote is the recap — reason
        // included — not the bare name-only shape.
        assert_eq!(
            failure_recap("MP4 export failed: ffmpeg exited with status 1", "MP4"),
            "MP4 export failed: ffmpeg exited with status 1"
        );
        // A status that is not a failure wording never
        // leaks into a failure recap.
        assert_eq!(failure_recap("Exported 5 frames", "MP4"), "MP4 export failed");
        assert_eq!(failure_recap("", "WebM"), "WebM export failed");
    }

    #[test]
    fn recap_severity_follows_the_toast_classifier() {
        assert_eq!(
            recap_severity("Exported PNG → ./export/"),
            Severity::Success
        );
        assert_eq!(
            recap_severity("Exported WebM (frames 1–5, video only) → ./export/animation.webm"),
            Severity::Success
        );
        assert_eq!(
            recap_severity("MP4 export failed: ffmpeg exited with status 1"),
            Severity::Error
        );
        // A partially failed bundle names its failures in
        // the recap, so it reads Error — the same call
        // `classify_status` makes for the bundle's toast
        // (audit D11: acceptable, and pinned here so it
        // stays a decision, not an accident).
        assert_eq!(
            recap_severity("Exported WebM → ./export/animation.webm; failed: MP4"),
            Severity::Error
        );
        assert_eq!(
            recap_severity("Exported nothing; failed: MP4, WebM"),
            Severity::Error
        );
    }

    #[test]
    fn output_text_states_frames_dimensions_and_fps() {
        let mut core = EditorCore::new();
        let n = core.project.frames.len();
        let [w, h] = core.project.stage;
        let expected = format!(
            "Output: {n} frames · {}×{} · {} fps",
            fmt_measure(w),
            fmt_measure(h),
            fmt_measure(core.project.fps)
        );
        assert_eq!(output_text(&core, false), expected);
        // Toggle on but no loop range: the fallback is the
        // full count, exactly as `effective_text` admits.
        assert_eq!(output_text(&core, true), expected);
        // With a loop range the count is the range's.
        core.loop_range = Some((0, 1));
        assert!(output_text(&core, true).starts_with("Output: 2 frames · "));
        assert_eq!(output_text(&core, false), expected, "toggle off ignores the range");
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
        // Wave T (T1): the buckets render through the
        // catalogue now, so the test drives them with an
        // English core — the strings asserted are the
        // pre-wire literals, byte for byte (they hold
        // once IA-T1-1 lands the keys; the Q7 pattern).
        let core = EditorCore::new();
        let now = at(10 * 24 * 60 * 60);
        assert_eq!(age_label(&core, now, now), "just now");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 59)), "just now");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 60)), "1 min ago");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 45 * 60)), "45 min ago");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 60 * 60)), "1 h ago");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 23 * 60 * 60)), "23 h ago");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 24 * 60 * 60)), "1 d ago");
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 - 3 * 24 * 60 * 60)), "3 d ago");
        // A future mtime (clock skew) reads as "just now",
        // never as a negative age.
        assert_eq!(age_label(&core, now, at(10 * 24 * 60 * 60 + 3600)), "just now");
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
            (PathBuf::from("/x/export/old.gif"), 10, at(100), 1),
            (PathBuf::from("/x/export/b.png"), 10, at(300), 1),
            (PathBuf::from("/x/out/a.apng"), 10, at(300), 1),
            (PathBuf::from("/x/export/new.mp4"), 10, at(500), 1),
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
            .map(|i| {
                (
                    PathBuf::from(format!("/x/export/f{i}.png")),
                    1,
                    at(1000 + i),
                    1,
                )
            })
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
    fn sequence_frame_names_match_the_writers_pattern() {
        assert!(is_sequence_frame(Path::new("/x/export/frame-0001.png")));
        assert!(is_sequence_frame(Path::new("/x/out/jpeg/frame-0012.jpg")));
        assert!(is_sequence_frame(Path::new("frame-0003.svg")));
        // Not sequence frames: the bare word, a hand-named
        // draft, and ordinary exports keep their own rows.
        assert!(!is_sequence_frame(Path::new("/x/export/frame.png")));
        assert!(!is_sequence_frame(Path::new("/x/export/frame-draft.png")));
        assert!(!is_sequence_frame(Path::new("/x/export/animation.mp4")));
        assert!(!is_sequence_frame(Path::new("/x/export/spritesheet.png")));
    }

    #[test]
    fn sequence_frames_collapse_into_one_row_per_directory() {
        let triples = vec![
            (PathBuf::from("/x/export/frame-0001.png"), 100, at(100), 1),
            (PathBuf::from("/x/export/frame-0002.png"), 200, at(300), 1),
            (PathBuf::from("/x/export/frame-0003.png"), 300, at(200), 1),
            (PathBuf::from("/x/export/animation.gif"), 50, at(400), 1),
        ];
        let rows = rows_from_triples(&triples);
        assert_eq!(rows.len(), 2, "three frames + the GIF: {rows:?}");
        // Newest first: the GIF, then the collapsed row
        // carrying the total size, the newest frame's
        // mtime and the frame count.
        assert_eq!(rows[0].name, "export/animation.gif");
        assert_eq!(rows[1].name, "export/frame-*.png");
        assert_eq!(rows[1].size, 600);
        assert_eq!(rows[1].mtime, at(300));
        assert_eq!(rows[1].count, 3);
        // The row line names the count, so the asterisk
        // name cannot be mistaken for a literal file.
        assert_eq!(
            row_line(&EditorCore::new(), &rows[1], at(300)),
            "export/frame-*.png · 3 files · 600 B · just now"
        );
    }

    #[test]
    fn a_lone_frame_file_keeps_its_own_name() {
        // One frame is not a flood; collapsing it would
        // also misname the only file it stands for.
        let triples = vec![(PathBuf::from("/x/export/frame-0001.png"), 100, at(100), 1)];
        let rows = rows_from_triples(&triples);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "export/frame-0001.png");
        assert_eq!(rows[0].count, 1);
    }

    #[test]
    fn sequence_directories_arrive_as_single_counted_rows() {
        // The scan's directory triples (path = the
        // directory itself, count = its file count) pass
        // through the row model untouched and render with
        // their counts — this is how JPEG and SVG exports
        // appear in the history at all.
        let triples = vec![
            (PathBuf::from("/x/out/jpeg"), 4096, at(500), 24),
            (PathBuf::from("/x/export/svg"), 2048, at(600), 12),
        ];
        let rows = rows_from_triples(&triples);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "export/svg");
        assert_eq!(rows[0].count, 12);
        assert_eq!(rows[1].name, "out/jpeg");
        assert_eq!(rows[1].count, 24);
        assert_eq!(
            row_line(&EditorCore::new(), &rows[1], at(500 + 60)),
            "out/jpeg · 24 files · 4.0 KB · 1 min ago"
        );
    }

    #[test]
    fn row_line_combines_name_size_and_age() {
        let row = ExportRow {
            name: "export/animation.mp4".into(),
            size: 1536,
            mtime: at(1000),
            count: 1,
        };
        assert_eq!(
            row_line(&EditorCore::new(), &row, at(1000 + 2 * 60 * 60)),
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
        // The wording names the Wave W (W6) truth
        // exactly: range MP4 and range WebM both keep
        // clip audio inside the range, WAV or any other
        // ffmpeg-readable format (AAC in the MP4, Opus
        // in the WebM); a clip whose file cannot be
        // probed or decoded fails the export instead of
        // being left out silently. Only a full WebM
        // export is silent (`export::export_webm` never
        // muxes) — the note is range-only copy, so it
        // does not say that either way.
        assert_eq!(
            RANGE_AUDIO_NOTE,
            "Range MP4 and WebM exports include audio from WAV and other ffmpeg-readable clips cut to the range — a clip whose file cannot be probed or decoded fails the export instead of being left out silently."
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

    // ---- .piv media note (Wave 12) ----

    /// Give the demo project's first figure a text segment
    /// on its second node — the segment-media fixture.
    fn with_segment_media(core: &mut EditorCore) {
        assert!(
            core.project.figures[0].def.nodes.len() > 1,
            "demo figure premise: it has a second node"
        );
        core.project.figures[0].def.nodes[1].media =
            Some(stykanimate_rs::model::SegmentMedia::Text {
                content: "Hi".into(),
                size: 20.0,
            });
    }

    /// A text-instance fixture (its keys are irrelevant —
    /// the note only reads that instances exist).
    fn text_instance() -> stykanimate_rs::model::TextInstance {
        stykanimate_rs::model::TextInstance {
            content: "Title".into(),
            size: 24.0,
            color: 0xFFFFFF,
            keys: Vec::new(),
        }
    }

    #[test]
    fn piv_media_note_hidden_for_the_demo_project() {
        // Premise: the demo project carries no segment
        // media, no sprite instances and no text instances,
        // so no .piv export drops anything — the note stays
        // hidden and the panel geometry is unchanged.
        let core = EditorCore::new();
        assert!(!project_has_segment_media(&core), "demo premise");
        assert!(core.project.sprites.is_empty(), "demo premise");
        assert!(core.project.texts.is_empty(), "demo premise");
        assert_eq!(piv_media_note(&core), None);
    }

    #[test]
    fn piv_media_note_names_the_plain_segment_drop() {
        // Segment media only: the note must name the
        // v2/v3/v4 plain-segment drop and v5 as the keeper,
        // and must not claim an instance drop the project
        // does not have.
        let mut core = EditorCore::new();
        with_segment_media(&mut core);
        assert!(project_has_segment_media(&core));
        assert_eq!(
            piv_media_note(&core).as_deref(),
            Some(
                "Media note: .piv v2/v3/v4 export image and text segments as plain segments — only v5 keeps segment media. Not even it keeps a media's local payload: the transform and opacity stay behind."
            )
        );
    }

    #[test]
    fn piv_media_note_names_the_instance_drop_for_a_sprite() {
        let mut core = EditorCore::new();
        core.project
            .sprites
            .push(stykanimate_rs::model::SpriteInstance {
                asset: 0,
                keys: Vec::new(),
            });
        assert_eq!(
            piv_media_note(&core).as_deref(),
            Some(
                "Media note: no .piv version writes sprite or text instances — they are left out of .piv exports."
            )
        );
    }

    #[test]
    fn piv_media_note_names_the_instance_drop_for_a_text_instance() {
        let mut core = EditorCore::new();
        core.project.texts.push(text_instance());
        assert_eq!(
            piv_media_note(&core).as_deref(),
            Some(
                "Media note: no .piv version writes sprite or text instances — they are left out of .piv exports."
            )
        );
    }

    #[test]
    fn piv_media_note_combines_both_drops() {
        let mut core = EditorCore::new();
        with_segment_media(&mut core);
        core.project.texts.push(text_instance());
        assert_eq!(
            piv_media_note(&core).as_deref(),
            Some(
                "Media note: .piv v2/v3/v4 export image and text segments as plain segments (only v5 keeps them), and no .piv version writes sprite or text instances — they are left out of .piv exports. Neither keeps a media's local payload: the transform and opacity stay behind."
            )
        );
    }

    // ---- Sprite note (Wave 13) ----

    /// A sprite-instance fixture (its keys are
    /// irrelevant — the note only reads that instances
    /// exist).
    fn sprite_instance() -> stykanimate_rs::model::SpriteInstance {
        stykanimate_rs::model::SpriteInstance {
            asset: 0,
            keys: Vec::new(),
        }
    }

    #[test]
    fn sprite_note_hidden_for_the_demo_project() {
        // Premise: the demo project has no sprite
        // instances, so no figure-file export drops any —
        // the note stays hidden and the panel geometry is
        // unchanged.
        let core = EditorCore::new();
        assert!(!project_has_sprites(&core), "demo premise");
        assert_eq!(sprite_note(&core), None);
    }

    #[test]
    fn sprite_note_hidden_with_only_text_instances() {
        // The note is the sprite half only: text
        // instances alone do not trigger it (their .piv
        // drop is the media note's clause, not this
        // note's).
        let mut core = EditorCore::new();
        core.project.texts.push(text_instance());
        assert!(!project_has_sprites(&core));
        assert_eq!(sprite_note(&core), None);
    }

    #[test]
    fn sprite_note_hidden_with_only_segment_media() {
        // Segment media rides on figures, not on the
        // project, so it is not a sprite instance and
        // does not trigger the sprite note.
        let mut core = EditorCore::new();
        with_segment_media(&mut core);
        assert!(!project_has_sprites(&core));
        assert_eq!(sprite_note(&core), None);
    }

    #[test]
    fn sprite_note_names_the_figure_file_drops() {
        // With a sprite instance present the note must
        // name the figure-file formats that leave it
        // behind — the two single-figure writers (.stk,
        // .nodes) — and state the .stknds truth A5 landed
        // in Wave 13: the whole-project writer carries
        // the sprite tracks. It must not claim a .piv
        // drop — the media note under the .piv rows
        // already states that half.
        let mut core = EditorCore::new();
        core.project.sprites.push(sprite_instance());
        assert!(project_has_sprites(&core));
        let note = sprite_note(&core).expect("note with a sprite instance present");
        assert_eq!(
            note,
            "Sprite note: .stk and .nodes carry only the selected figure, so project sprite instances are left out of those two exports. .stknds carries the sprite tracks (on re-import, groups come back as plain images and text instances are not carried)."
        );
        assert!(!note.contains(".piv"), "the .piv half is the media note's: {note}");
    }

    #[test]
    fn progress_labels_exist_in_the_catalogue_and_match_english_byte_identically() {
        // Wave W (W1, IA-W1-1): the progress readout's
        // keys — the working line and the two phase
        // templates in stored form (slots included),
        // plus the encode-finished sentence; Wave Z
        // (Z8, IA-Z8-1) adds the bundle template. Like the
        // template half of the pin above, this passes
        // only once the integrator inserts the keys
        // (the Q7 pattern): until then `tr` falls back
        // to the key text and `key_in_table` is false.
        let core = EditorCore::new();
        for (key, literal) in [
            ("export_progress_working", "Exporting {name}…"),
            ("export_render_progress", "Rendering {done} of {total} frames"),
            ("export_encode_progress", "Encoding {done} of {total} frames"),
            ("export_encode_finished", "Encode finished"),
            // Wave Z (Z8, IA-Z8-1): the bundle line's
            // template, in stored form.
            (
                "export_bundle_progress",
                "Exporting bundle {done} of {total} frames",
            ),
        ] {
            assert!(
                crate::rune_sec::i18n_wire::key_in_table(key),
                "{key} must exist in the i18n table"
            );
            assert_eq!(
                crate::rune_sec::i18n_wire::tr(&core, key),
                literal,
                "{key} must render byte-identically in English"
            );
        }
    }

    // ---- Export jobs (Wave W, W1) ----
    // The job machinery's state transitions, driven by
    // test workers through `begin_job`'s work seam: no
    // pin runs a real export entry or writes an export
    // file (the destinations below are inert strings the
    // test workers never touch).

    /// Poll until the job in `s` finishes — bounded, so
    /// a broken drain fails the pin instead of hanging
    /// the suite.
    fn poll_until_finished(s: &mut State) -> (bool, bool) {
        for _ in 0..2000 {
            let drained = poll_export_job(s);
            if !drained.1 {
                return drained;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("export job did not finish");
    }

    /// Begin a job whose worker blocks until the
    /// returned sender is signalled, then returns
    /// `result` as its outcome.
    fn blocked_job(s: &mut State, kind: JobKind, result: ExportJobOutcome) -> mpsc::Sender<()> {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work: JobWork = Box::new(move |_project, _render, _encode, _bundle| {
            let _ = release_rx.recv();
            result
        });
        let project = s.core.project.clone();
        begin_job(s, kind, PathBuf::from("out-test"), project, work);
        release_tx
    }

    #[test]
    fn job_poll_redraws_only_when_a_phase_snapshot_changes() {
        let mut s = crate::init();
        let (signal_tx, signal_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work: JobWork = Box::new(move |_project, render, _encode, _bundle| {
            let _ = signal_rx.recv();
            render.store_render(stykanimate_rs::export::RenderProgressEvent {
                rendered_frames: 2,
                total_frames: 5,
                finished: false,
            });
            let _ = release_rx.recv();
            Ok(ExportJobResult::Frames(5))
        });
        let project = s.core.project.clone();
        begin_job(&mut s, JobKind::PngFull, PathBuf::from("out-test"), project, work);
        // Nothing reported yet: active, no repaint — and
        // the readout's only line is the working line
        // (no phase shows a number it never delivered).
        assert_eq!(poll_export_job(&mut s), (false, true));
        let job = s.export_job.as_ref().expect("job running");
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-working");
        // A stored event is a change: repaint, still
        // active.
        signal_tx.send(()).expect("worker listening");
        let mut drained = (false, true);
        for _ in 0..2000 {
            drained = poll_export_job(&mut s);
            if drained.0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(drained, (true, true));
        // No new event since: no repaint, still active.
        assert_eq!(poll_export_job(&mut s), (false, true));
        release_tx.send(()).expect("worker listening");
        assert_eq!(poll_until_finished(&mut s), (true, false));
        assert!(s.export_job.is_none());
    }

    #[test]
    fn finished_range_png_job_writes_the_actions_status_and_recap() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::PngRange(1, 3),
            Ok(ExportJobResult::Frames(3)),
        );
        release.send(()).expect("worker listening");
        assert_eq!(poll_until_finished(&mut s), (true, false));
        // The range action's exact sentence, the range
        // recap — produced at finish, not by the press.
        assert_eq!(s.core.status, "Exported 3 PNG frames (frames 2–4) to out-test");
        assert_eq!(
            s.export_recap,
            single_recap_range("PNG", "./export/", (1, 3), false)
        );
        // A later poll is inert.
        assert_eq!(poll_export_job(&mut s), (false, false));
    }

    #[test]
    fn failed_job_keeps_the_failure_wording_and_reason() {
        let mut s = crate::init();
        let release = blocked_job(&mut s, JobKind::Mp4Full, Err("boom".to_string()));
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        assert_eq!(s.core.status, "MP4 export failed: boom");
        // The recap keeps the status verbatim — reason
        // included — via `failure_recap`'s convention.
        assert_eq!(s.export_recap, "MP4 export failed: boom");
    }

    #[test]
    fn mp4_finish_names_missing_audio_like_the_full_op() {
        let mut s = crate::init();
        let outcome = stykanimate_rs::export::Mp4ExportOutcome {
            frames: 5,
            missing_audio: vec!["gone.wav".to_string()],
        };
        let note = outcome.missing_audio_note().expect("a skipped clip is named");
        let release = blocked_job(
            &mut s,
            JobKind::Mp4Full,
            Ok(ExportJobResult::Mp4(outcome)),
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        assert_eq!(
            s.core.status,
            format!("Exported 5-frame MP4 to out-test. {note}")
        );
    }

    #[test]
    fn webm_range_finish_keeps_the_class_c_sentence_with_an_outcome() {
        // Wave X (X12): the range WebM job now carries the
        // shared outcome — but the finish sentence is the
        // class-C wording of record, so it stays
        // byte-identical even when the outcome names a
        // skipped clip (the note composition belongs to
        // the MP4 sentences; the standing exclusion owns
        // this wording).
        let mut s = crate::init();
        let outcome = stykanimate_rs::export::Mp4ExportOutcome {
            frames: 5,
            missing_audio: vec!["gone.wav".to_string()],
        };
        let release = blocked_job(
            &mut s,
            JobKind::WebmRange(0, 4),
            Ok(ExportJobResult::Webm(outcome)),
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        assert_eq!(
            s.core.status,
            "Exported WebM (frames 1–5, video only) to out-test"
        );
    }

    #[test]
    fn phases_stay_separate_in_the_slots_and_the_lines() {
        let mut s = crate::init();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work: JobWork = Box::new(move |_project, render, encode, _bundle| {
            render.store_render(stykanimate_rs::export::RenderProgressEvent {
                rendered_frames: 2,
                total_frames: 5,
                finished: false,
            });
            encode.store_encode(stykanimate_rs::export::EncodeProgressEvent {
                encoded_frames: 1,
                total_frames: 9,
                finished: false,
            });
            let _ = release_rx.recv();
            Ok(ExportJobResult::Webm(
                stykanimate_rs::export::Mp4ExportOutcome {
                    frames: 5,
                    missing_audio: Vec::new(),
                },
            ))
        });
        let project = s.core.project.clone();
        begin_job(
            &mut s,
            JobKind::WebmRange(0, 4),
            PathBuf::from("out-test"),
            project,
            work,
        );
        // Wait until both stores are visible.
        for _ in 0..2000 {
            let job = s.export_job.as_ref().expect("job running");
            if job.render.snapshot().seen && job.encode.snapshot().seen {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let job = s.export_job.as_ref().expect("job running");
        let render = job.render.snapshot();
        let encode = job.encode.snapshot();
        // Each slot kept its own event's numbers — the
        // render store did not move the encode counter
        // or vice versa.
        assert_eq!((render.done, render.total), (2, 5));
        assert_eq!((encode.done, encode.total), (1, 9));
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, "xc-progress-render");
        assert_eq!(lines[0].1, render_progress_line(&s.core, render));
        assert_eq!(lines[1].0, "xc-progress-encode");
        assert_eq!(lines[1].1, encode_progress_line(&s.core, encode));
        release_tx.send(()).expect("worker listening");
        poll_until_finished(&mut s);
    }

    #[test]
    fn encode_terminal_reads_as_encode_finished_only() {
        let mut s = crate::init();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work: JobWork = Box::new(move |_project, _render, encode, _bundle| {
            encode.store_encode(stykanimate_rs::export::EncodeProgressEvent {
                encoded_frames: 9,
                total_frames: 9,
                finished: true,
            });
            let _ = release_rx.recv();
            Ok(ExportJobResult::Frames(9))
        });
        let project = s.core.project.clone();
        begin_job(
            &mut s,
            JobKind::WebmFull,
            PathBuf::from("out-test"),
            project,
            work,
        );
        for _ in 0..2000 {
            match s.export_job.as_ref() {
                Some(job) if job.encode.snapshot().seen => break,
                _ => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        let job = s.export_job.as_ref().expect("job running");
        let lines = job_lines(&s.core, job);
        // The terminal encode line claims the encode
        // finished — and while the job is still active
        // (no outcome yet) nothing claims the export
        // itself finished: the status is untouched.
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-encode");
        assert_eq!(
            lines[0].1,
            crate::rune_sec::i18n_wire::tr(&s.core, "export_encode_finished")
        );
        assert!(export_job_active(&s));
        assert!(!s.core.status.contains("Exported"));
        release_tx.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        assert_eq!(s.core.status, "Exported 9-frame WebM.");
    }

    #[test]
    fn a_second_press_is_a_no_op_while_a_job_runs() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::PngFull,
            Ok(ExportJobResult::Frames(1)),
        );
        assert!(export_job_active(&s));
        // A contracted press refuses while a job runs...
        assert!(!start_export_job(&mut s, ExportOp::Mp4));
        // ...and the synchronous presses are inert too:
        // the sentinel status survives an SVG press and
        // a bundle press untouched.
        s.core.status = "sentinel".to_string();
        export_svg_action(&mut s);
        assert_eq!(s.core.status, "sentinel");
        run_bundle(&mut s, BundleKind::Images);
        assert_eq!(s.core.status, "sentinel");
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        // The original job — not any refused press —
        // wrote the finish.
        assert_eq!(s.core.status, "Exported 1 PNG frames to out-test");
    }

    #[test]
    fn a_worker_that_never_reports_finishes_as_a_failure() {
        let mut s = crate::init();
        let (tx, rx) = mpsc::channel::<ExportJobOutcome>();
        drop(tx);
        s.export_job = Some(ExportJob {
            kind: JobKind::WebmFull,
            dest: PathBuf::from("out-test"),
            render: Arc::new(PhaseProgress::default()),
            encode: Arc::new(PhaseProgress::default()),
            bundle: Arc::new(PhaseProgress::default()),
            outcome: rx,
            shown_render: PhaseSnapshot::default(),
            shown_encode: PhaseSnapshot::default(),
            shown_bundle: PhaseSnapshot::default(),
        });
        assert_eq!(poll_export_job(&mut s), (true, false));
        assert!(s.export_job.is_none());
        assert_eq!(
            s.core.status,
            "WebM export failed: the export worker stopped before reporting a result"
        );
    }

    #[test]
    fn job_spec_matches_the_actions_destinations() {
        let mut s = crate::init();
        s.export_use_range = false;
        let (kind, dest) = job_spec(&s, ExportOp::Png).expect("PNG is contracted");
        assert_eq!(kind, JobKind::PngFull);
        assert_eq!(dest, export_dir());
        let (kind, dest) = job_spec(&s, ExportOp::Jpeg).expect("JPEG is contracted");
        assert_eq!(kind, JobKind::JpegFull);
        assert_eq!(dest, PathBuf::from("out").join("jpeg"));
        // With a loop range set and the toggle on, the
        // range is baked into the kind and the video
        // destination gains its file name.
        s.export_use_range = true;
        s.core.loop_range = Some((0, 2));
        let (kind, dest) = job_spec(&s, ExportOp::Mp4).expect("MP4 is contracted");
        assert_eq!(kind, JobKind::Mp4Range(0, 2));
        assert_eq!(dest, export_dir().join("animation.mp4"));
        // Uncontracted formats are never jobbed.
        assert!(job_spec(&s, ExportOp::Svg).is_none());
        assert!(job_spec(&s, ExportOp::Gif).is_none());
    }

    // ---- Bundle jobs (Wave Z, Z8) ----
    // The bundle seats of the same machinery, driven by
    // test workers through the same seam: no pin runs a
    // real member entry or writes an export file.

    #[test]
    fn bundle_job_spec_maps_kinds_and_the_destination_root() {
        let (kind, dest) = bundle_job_spec(BundleKind::Images);
        assert_eq!(kind, JobKind::BundleImages);
        assert_eq!(dest, export_dir());
        let (kind, dest) = bundle_job_spec(BundleKind::Video);
        assert_eq!(kind, JobKind::BundleVideo);
        assert_eq!(dest, export_dir());
        // The kind round-trips to its bundle, and no
        // single-format kind claims one.
        assert_eq!(JobKind::BundleImages.bundle_kind(), Some(BundleKind::Images));
        assert_eq!(JobKind::BundleVideo.bundle_kind(), Some(BundleKind::Video));
        assert_eq!(JobKind::Mp4Full.bundle_kind(), None);
        assert_eq!(JobKind::PngRange(0, 1).bundle_kind(), None);
    }

    #[test]
    fn bundle_member_totals_are_allocated_before_any_member_runs() {
        let mut s = crate::init();
        // Images: one share per member, each the
        // authored frame count, in plan order.
        let n = s.core.project.frames.len();
        assert_eq!(
            bundle_member_totals(BundleKind::Images, &s.core.project),
            vec![n; 4]
        );
        // Video: two equal shares, each the core's
        // sampled-times total (the encode contract's
        // own count — the lib fn is the oracle; its
        // arithmetic is X11's pins).
        let member = stykanimate_rs::export::sampled_frame_total(&s.core.project);
        assert_eq!(
            bundle_member_totals(BundleKind::Video, &s.core.project),
            vec![member, member]
        );
        // A longer timeline scales the images shares
        // frame for frame.
        let target = n + 2;
        while s.core.project.frames.len() < target {
            let frame = s.core.project.frames[0].clone();
            s.core.project.frames.push(frame);
        }
        assert_eq!(
            bundle_member_totals(BundleKind::Images, &s.core.project),
            vec![target; 4]
        );
    }

    #[test]
    fn bundle_job_finish_writes_the_recap_to_status_and_recap() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundleImages,
            Ok(ExportJobResult::Bundle(vec![ExportOp::Svg])),
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        // The synchronous handler's last word, verbatim:
        // the recap IS the status, and the same line is
        // the stored recap — the failed member named,
        // the rest claimed with their destinations.
        let recap = bundle_recap(BundleKind::Images, &[ExportOp::Svg]);
        assert_eq!(
            recap,
            "Exported PNG → ./export/, JPEG → out/jpeg/, sheet → ./export/spritesheet.png; failed: SVG"
        );
        assert_eq!(s.core.status, recap);
        assert_eq!(s.export_recap, recap);
    }

    #[test]
    fn bundle_job_with_every_member_succeeding_recaps_the_whole_plan() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundleVideo,
            Ok(ExportJobResult::Bundle(Vec::new())),
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        let recap = bundle_recap(BundleKind::Video, &[]);
        assert_eq!(recap, "Exported MP4 → ./export/animation.mp4, WebM → ./export/animation.webm");
        assert_eq!(s.core.status, recap);
        assert_eq!(s.export_recap, recap);
    }

    #[test]
    fn bundle_worker_that_never_reports_fails_every_member() {
        let mut s = crate::init();
        let (tx, rx) = mpsc::channel::<ExportJobOutcome>();
        drop(tx);
        s.export_job = Some(ExportJob {
            kind: JobKind::BundleImages,
            dest: PathBuf::from("out-test"),
            render: Arc::new(PhaseProgress::default()),
            encode: Arc::new(PhaseProgress::default()),
            bundle: Arc::new(PhaseProgress::default()),
            outcome: rx,
            shown_render: PhaseSnapshot::default(),
            shown_encode: PhaseSnapshot::default(),
            shown_bundle: PhaseSnapshot::default(),
        });
        assert_eq!(poll_export_job(&mut s), (true, false));
        assert!(s.export_job.is_none());
        // No member outcome was reported, so the recap
        // claims nothing: every member named as failed,
        // in the bundle's own voice — never a success
        // and never a single-format failure sentence.
        let recap = bundle_recap(BundleKind::Images, &plan_bundle(BundleKind::Images));
        assert_eq!(recap, "Exported nothing; failed: PNG, JPEG, SVG, sheet");
        assert_eq!(s.core.status, recap);
        assert_eq!(s.export_recap, recap);
    }

    #[test]
    fn bundle_slot_drives_its_own_line_and_stays_out_of_the_phases() {
        let mut s = crate::init();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work: JobWork = Box::new(move |_project, render, _encode, bundle| {
            bundle.store_bundle(stykanimate_rs::export::BundleProgressEvent {
                completed_frames: 3,
                total_frames: 8,
                finished: false,
            });
            // A stray phase store on a bundle job moves
            // only its own slot — the readout must not
            // surface it (a bundle job has no phase
            // lines) and the bundle count must not move.
            render.store_render(stykanimate_rs::export::RenderProgressEvent {
                rendered_frames: 2,
                total_frames: 5,
                finished: false,
            });
            let _ = release_rx.recv();
            Ok(ExportJobResult::Bundle(Vec::new()))
        });
        let project = s.core.project.clone();
        begin_job(
            &mut s,
            JobKind::BundleImages,
            PathBuf::from("out-test"),
            project,
            work,
        );
        // The bundle store is a snapshot change: the
        // poll asks for a repaint while the job runs.
        let mut drained = (false, true);
        for _ in 0..2000 {
            drained = poll_export_job(&mut s);
            if drained.0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(drained, (true, true));
        let job = s.export_job.as_ref().expect("job running");
        // Each slot kept its own event's numbers.
        let bundle = job.bundle.snapshot();
        assert_eq!((bundle.done, bundle.total, bundle.seen), (3, 8, true));
        assert_eq!(job.render.snapshot().done, 2);
        // The readout is exactly the one bundle line,
        // built from the bundle snapshot — no phase
        // line, no merged figure.
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-bundle");
        assert_eq!(lines[0].1, bundle_progress_line(&s.core, bundle));
        release_tx.send(()).expect("worker listening");
        poll_until_finished(&mut s);
    }

    #[test]
    fn bundle_job_before_any_event_shows_the_working_line() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundleVideo,
            Ok(ExportJobResult::Bundle(Vec::new())),
        );
        let job = s.export_job.as_ref().expect("job running");
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-working");
        assert_eq!(
            lines[0].1,
            crate::rune_sec::i18n_wire::tr(&s.core, "export_progress_working")
                .replace("{name}", "video bundle")
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
    }

    #[test]
    fn bundle_press_is_a_no_op_while_a_job_runs() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundleImages,
            Ok(ExportJobResult::Bundle(Vec::new())),
        );
        // A second bundle press refuses, and so does a
        // contracted single-format press: the running
        // job is untouched either way.
        assert!(!start_bundle_job(&mut s, BundleKind::Video));
        assert!(!start_export_job(&mut s, ExportOp::Png));
        s.core.status = "sentinel".to_string();
        run_bundle(&mut s, BundleKind::Video);
        assert_eq!(s.core.status, "sentinel");
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        // The original bundle job wrote the finish.
        assert_eq!(
            s.core.status,
            bundle_recap(BundleKind::Images, &[])
        );
    }
}
