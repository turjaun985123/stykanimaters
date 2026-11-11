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
//!
//! Roadmap Wave 1 (RM23, P-13) adds export delivery.
//! FIRST: named destinations + an overwrite guard.
//! Every single-file destination the panel controls
//! is now derived from the project name
//! (`project_stem` / `ExportOp::dest_named`): the MP4
//! master is `./export/<project>.mp4`, not a shared
//! `animation.mp4` the next episode silently replaces.
//! And before any export whose single-file targets
//! already exist, the first press only arms the guard
//! and says so (status + recap, one new sentence —
//! no existing class-C sentence changes); the second
//! press of the same export overwrites. The guard
//! covers the jobbed MP4/WebM, the bundles' file
//! members, the synchronous GIF/APNG/sheet/contact
//! files, the .piv/.stknds files and the delivery /
//! publish destinations; the sequence formats (PNG/
//! JPEG/SVG directories of regenerable frames) stay
//! unguarded, as do .stk/.nodes (their file names are
//! the figure's, chosen inside the editor ops). The
//! full `EditorCore` ops for GIF/sheet/APNG still
//! write their hardcoded names — the panel now calls
//! the lib entries directly at the named paths in
//! both modes, in the ops' exact status wording, so
//! the guard and the naming hold in "All frames" too.
//! THEN: a Delivery section — preset chips (Project
//! master / YouTube 1080p / YouTube 4K / Community
//! GIF) composing format + the core scaled entries,
//! one Export delivery button, and its honest output
//! line naming preset, dimensions and destination.
//! (A Shorts preset (Roadmap Wave 2, W2-43,
//! IA-RM23-4 landed) crops instead of scaling: the
//! core entry renders at 1920 tall and cuts the
//! centred 9:16 window — centred only, no subject
//! tracking, so the note under the delivery line says
//! what the crop keeps.) And a third bundle, "Publish
//! package": the delivery MP4 (the selected MP4
//! preset's leg) + the SRT and VTT sidecars from the
//! subtitle writers + a 1280-wide thumbnail still of
//! the playhead frame, run as one job on the bundle
//! contract, its recap naming every member. Export
//! cancel stays unbuilt (decision D-4).
//!
//! Roadmap Wave 1 (RM29, P-16) adds the post-export QA
//! probe and the partial-artifact honesty. When an
//! MP4/WebM job (single, delivery, video bundle or the
//! publish package's MP4 leg) finishes successfully,
//! the produced file is probed — ffprobe stream and
//! duration data, a full audio decode metered for peak
//! and RMS, a black-frame scan over sampled frames and,
//! when the project has subtitle cues in the exported
//! window, a burn-in spot-check counting text-colour
//! ink in the anchored band — and the verdict renders
//! as its own line under the recap (`xc-qa`), built by
//! `qa_verdict` from the measurements alone. The probe
//! is detectors only: it never writes or rewrites the
//! status or the recap, and no class-C sentence changes
//! — a failed probe is a second, separate record, so
//! the P0-1 shape (a success sentence over a broken
//! file) now has a loud neighbour. Its sharpest check
//! is the silent-audio one: when the project has
//! audible clips the export claims, a decoded peak at
//! digital silence fails the verdict. The honesty half:
//! a failed PNG/JPEG sequence job states its residue —
//! the frame files the attempt left in the destination
//! — on its own line (`xc-residue`), and Recent exports
//! marks a sequence row incomplete when the directory
//! holds fewer frames than the last sequence job for
//! that row was expected to write. The QA and residue
//! labels ride IA-RM29-1; until the integrator inserts
//! them the wire falls back to the key text, the Q7
//! pattern.
//!
//! Roadmap Wave 2 (W2-44) moves the probe itself off
//! the UI thread (RM29 §7's recorded remainder): the
//! finish path now only reads the expectations off
//! the project and queues the probe (`queue_probe`);
//! a worker thread runs `probe_export` and returns
//! the plain-data report over a channel, and the
//! frame tick's drain (`poll_export_job`) stores it —
//! the same spawn/channel/drain pattern as the export
//! jobs above, no second job system. While a probe is
//! in flight the drain reads active, so `main::poll`
//! keeps the `JOB_WAKE` cadence until the verdict
//! lands. The probe has no cancel affordance: it is
//! short, read-only and detached, and D-4's bundle
//! cancel stays unbuilt — a probe worker that dies
//! without sending stores a failure report naming the
//! probe's own stoppage, the export worker-death
//! discipline one level down. Every landed report
//! also enters the session probe history
//! (`xc-qa-history`): the verdict line with the time
//! it landed and, on failure, the named checks —
//! session-only, like the reports themselves (RM29
//! specified no store surface for them).
//!
//! Roadmap Wave 2 (W2-64) completes the panel's
//! feedback surface (PL-11): every jobbed export now
//! carries a cancel affordance beside its progress
//! readout, rendered by the one builder
//! (`cancel_row`) the future stop build rewires. No
//! stop signal exists today — AC8 designed the core
//! `StopSignal` as a design only, and decision D-4
//! leaves its build (and the class-C wording around
//! a stopped bundle, AC8's Q1/Q2) to a user ruling —
//! so the affordance renders disabled-honest for
//! every job shape: the kit's inert button (no press
//! handler at all, AT-disabled) plus a visible note
//! saying the export runs to completion. It is never
//! a live-looking button that does nothing. The
//! seam is [`job_cancel`]: when the signal lands,
//! `ExportJob` gains its clone, `job_cancel` gains
//! an armed answer, and the exhaustive match in
//! `cancel_row` is the one place that must give it
//! a live button. The same seat gives the QA probe
//! the indeterminate half of the surface: while a
//! probe is in flight the panel shows one checking
//! line (`xc-qa-checking`) — a state, not a number,
//! since the probe exposes no phase contract
//! (W2-44) — where before it showed the previous
//! verdicts, or nothing, with no sign a check was
//! still running. The three new labels ride
//! IA-W2-64-1 (`export_cancel`,
//! `export_cancel_unavailable`,
//! `export_qa_checking`); until the integrator
//! inserts them the wire falls back to the key
//! text, the Q7 pattern.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Instant, SystemTime};

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::episode_qa::{EpisodeAudioScan, EpisodeCardExpectation, EpisodeChapterExpectation, EpisodeCheck, EpisodeCheckStatus, EpisodeCueFact, EpisodeExpectations, EpisodeFileChapter, EpisodeFrameSample, EpisodeMeasurements, EpisodeQaParams, EpisodeQaReport, QA_BLACK_LUMA, QA_INK_MIN, QA_SILENCE_PEAK, QaBurnIn, QaExpectations, VideoProbe, audio_scan, audio_stats, band_ink_count, burn_in_ink, centre_ink_count, display_name, episode_cue_coverage, episode_expectations, episode_qa_verdict, fmt_db, parse_ffmetadata_chapters, parse_ffprobe_chapters_json, parse_ffprobe_json, probe_episode, probe_video_file, qa_expectations, qa_temp_path};

use crate::chrome_view::{btn, btn_inert};
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

// ---- Overwrite guard (Roadmap Wave 1, RM23) -----------
// P-13's first item: an export must never silently
// replace a finished master. The guard is press-time
// and two-press: when any of the export's single-file
// targets already exists, the first press writes
// nothing — it arms the guard (the target set's
// signature, held in a thread-local like the history
// cache: panel session state, no `State` field) and
// reports through the status and the recap in one new
// sentence. A second press of the SAME export (the
// armed signature matches the targets that exist)
// proceeds and disarms. A press of a different export
// replaces the arm with its own guard. The sentence
// is a `core.status` record, so it stays English like
// every status record (the Wave T discipline); it is
// a NEW sentence — no class-C wording changes — and
// it is shaped so the detectors read it honestly: it
// does not start with "Exported " (not a success) and
// does not contain "failed" (not a failure).

thread_local! {
    /// The armed overwrite signature: the existing
    /// target paths of the last guarded press, joined
    /// by `overwrite_signature`.
    static OVERWRITE_ARM: RefCell<Option<String>> = RefCell::new(None);
}

/// Which of an export's targets already exist as
/// files — the guard's only filesystem read, one stat
/// per target. Pure apart from those stats.
pub fn existing_targets(targets: &[PathBuf]) -> Vec<PathBuf> {
    targets.iter().filter(|p| p.is_file()).cloned().collect()
}

/// The signature an arm is stored and matched under:
/// the existing targets' paths, in order.
pub fn overwrite_signature(existing: &[PathBuf]) -> String {
    existing
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("|")
}

/// The guard's sentence: the first existing target is
/// named, the rest are counted. (Status-record copy —
/// English by the class-C discipline, like the ops'
/// own sentences.)
pub fn overwrite_guard_line(existing: &[PathBuf]) -> String {
    let first = existing
        .first()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let more = if existing.len() > 1 {
        format!(" (and {} more)", existing.len() - 1)
    } else {
        String::new()
    };
    format!(
        "Export target already exists: {first}{more} — press the same export again to overwrite it."
    )
}

/// The press-time guard: returns true when the press
/// must NOT run — some target exists and this press
/// was not the confirming second press. On a block
/// the guard sentence is written to the status and
/// the recap (the panel's persistent record), so the
/// user sees exactly which file stopped the export.
fn guard_blocks(s: &mut State, targets: &[PathBuf]) -> bool {
    let existing = existing_targets(targets);
    if existing.is_empty() {
        return false;
    }
    let sig = overwrite_signature(&existing);
    let armed = OVERWRITE_ARM.with(|a| a.borrow().clone());
    if armed.as_deref() == Some(sig.as_str()) {
        OVERWRITE_ARM.with(|a| *a.borrow_mut() = None);
        return false;
    }
    OVERWRITE_ARM.with(|a| *a.borrow_mut() = Some(sig));
    let line = overwrite_guard_line(&existing);
    s.core.status = line.clone();
    s.export_recap = line;
    true
}

/// The single-file targets a bundle press would
/// write, under its destination root: the images
/// bundle's only file member is the spritesheet (the
/// sequences write directories); the video bundle's
/// two members are both files.
fn bundle_file_targets(kind: BundleKind, project: &stykanimate_rs::model::Project, root: &Path) -> Vec<PathBuf> {
    let stem = project_stem(project);
    match kind {
        BundleKind::Images => vec![root.join(
            file_name_for(ExportOp::Spritesheet, &stem).expect("spritesheet is a file format"),
        )],
        BundleKind::Video => [ExportOp::Mp4, ExportOp::Webm]
            .iter()
            .map(|op| root.join(file_name_for(*op, &stem).expect("video formats are files")))
            .collect(),
    }
}

// ---- Delivery presets (Roadmap Wave 1, RM23) ----------
// P-13's presets: one selection, composed over the
// machinery that already exists — the MP4/GIF scaled
// entries in `export.rs` (a true re-render at the
// scaled size, never an upscale) and the named
// destinations above. The selection is panel session
// state (the thread-local idiom of this file), so no
// `State` field is added; it starts at Project master
// on every launch. A Shorts preset (Roadmap Wave 2,
// W2-43, IA-RM23-4 landed) crops instead of scaling:
// the core entry renders at 1920 tall and cuts the
// centred 9:16 window — centred only, no subject
// tracking, so the note under the delivery line says
// what the crop keeps.

/// One delivery preset: a format plus a scale rule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeliveryPreset {
    /// The project as staged: MP4 at scale 1.0.
    Master,
    /// MP4 scaled so the picture is 1080 pt tall.
    Youtube1080,
    /// MP4 scaled so the picture is 2160 pt tall.
    Youtube4k,
    /// GIF at half scale — the community-post format.
    Gif,
    /// MP4 reframed to vertical video: the centred 9:16
    /// crop at 1920 tall (W2-43). Not a scale rule — the
    /// core Shorts entries own the geometry; the arms
    /// below that speak of scale answer for the frame
    /// BEFORE the crop wherever they must answer at all.
    Shorts,
}

impl DeliveryPreset {
    /// The catalogue key the preset's chip label
    /// renders through (IA-RM23-2).
    pub fn label_key(self) -> &'static str {
        match self {
            DeliveryPreset::Master => "export_preset_master",
            DeliveryPreset::Youtube1080 => "export_preset_youtube_1080",
            DeliveryPreset::Youtube4k => "export_preset_youtube_4k",
            DeliveryPreset::Gif => "export_preset_gif",
            DeliveryPreset::Shorts => "export_preset_shorts",
        }
    }

    /// Whether this preset's format is the GIF.
    pub fn is_gif(self) -> bool {
        matches!(self, DeliveryPreset::Gif)
    }

    /// The preset chip's element key.
    pub fn key(self) -> &'static str {
        match self {
            DeliveryPreset::Master => "xc-preset-master",
            DeliveryPreset::Youtube1080 => "xc-preset-yt1080",
            DeliveryPreset::Youtube4k => "xc-preset-yt4k",
            DeliveryPreset::Gif => "xc-preset-gif",
            DeliveryPreset::Shorts => "xc-preset-shorts",
        }
    }
}

thread_local! {
    /// The selected delivery preset (session state).
    static DELIVERY_PRESET: RefCell<DeliveryPreset> =
        RefCell::new(DeliveryPreset::Master);
}

/// The selected delivery preset.
pub fn delivery_preset() -> DeliveryPreset {
    DELIVERY_PRESET.with(|p| *p.borrow())
}

/// Select a delivery preset (the chips' handler).
pub fn set_delivery_preset(preset: DeliveryPreset) {
    DELIVERY_PRESET.with(|p| *p.borrow_mut() = preset);
}

/// The scale a preset renders at: the height presets
/// divide their target height by the stage height
/// (a stage already at the target renders at 1.0 and
/// routes through the unscaled entries inside the
/// core functions, byte-identical); the GIF preset
/// is a fixed half. A stage with no usable height
/// falls back to 1.0 rather than inventing a scale.
pub fn delivery_scale(project: &stykanimate_rs::model::Project, preset: DeliveryPreset) -> f32 {
    let for_height = |target: f32| {
        let h = project.stage[1];
        if h.is_finite() && h > 0.0 {
            target / h
        } else {
            1.0
        }
    };
    match preset {
        DeliveryPreset::Master => 1.0,
        DeliveryPreset::Youtube1080 => for_height(1080.0),
        DeliveryPreset::Youtube4k => for_height(2160.0),
        DeliveryPreset::Gif => 0.5,
        // Never consumed as a render scale: the Shorts
        // entries compute their own (1920/stage_h).
        // Present so the match stays total;
        // delivery_dims overrides below.
        DeliveryPreset::Shorts => 1.0,
    }
}

/// The pixel dimensions a preset will produce: the
/// stage dimensions times the preset's scale,
/// rounded, at least 1 per axis — the output line's
/// numbers, computed from the same scale the export
/// will use, so the two can never disagree.
pub fn delivery_dims(project: &stykanimate_rs::model::Project, preset: DeliveryPreset) -> (u32, u32) {
    if matches!(preset, DeliveryPreset::Shorts) {
        // The core geometry is the dims: the crop window
        // of the 1920-tall reframe (W2-43). An unusable
        // stage announces (0, 0) — the press then refuses
        // with the core's own error, never a fake size.
        return stykanimate_rs::export::shorts_geometry(project)
            .map(|g| g.crop)
            .unwrap_or((0, 0));
    }
    let scale = delivery_scale(project, preset);
    let dim = |v: f32| -> u32 {
        if v.is_finite() && v > 0.0 {
            (v * scale).round().max(1.0) as u32
        } else {
            0
        }
    };
    (dim(project.stage[0]), dim(project.stage[1]))
}

/// The file name a preset writes, from the project
/// stem: the height presets carry their platform
/// suffix (`<project>-1080p.mp4`, A12's R12.3 shape);
/// the master is the plain named MP4.
pub fn delivery_file_name(stem: &str, preset: DeliveryPreset) -> String {
    match preset {
        DeliveryPreset::Master => format!("{stem}.mp4"),
        DeliveryPreset::Youtube1080 => format!("{stem}-1080p.mp4"),
        DeliveryPreset::Youtube4k => format!("{stem}-4k.mp4"),
        DeliveryPreset::Gif => format!("{stem}.gif"),
        DeliveryPreset::Shorts => format!("{stem}-shorts.mp4"),
    }
}

/// Where a preset export writes: the export root
/// plus `delivery_file_name`.
pub fn delivery_dest(core: &EditorCore, preset: DeliveryPreset) -> PathBuf {
    export_dir().join(delivery_file_name(&project_stem(&core.project), preset))
}

/// The delivery line under the preset chips: what
/// the Export delivery press will produce — preset,
/// dimensions, destination — a catalogue template
/// (`export_delivery_line`, IA-RM23-2) with the
/// preset label, the dims and the destination as
/// slots, the camera `follow_label` idiom.
pub fn delivery_text(core: &EditorCore, preset: DeliveryPreset) -> String {
    let (w, h) = delivery_dims(&core.project, preset);
    i18n_wire::tr(core, "export_delivery_line")
        .replace("{preset}", &i18n_wire::tr(core, preset.label_key()))
        .replace("{width}", &w.to_string())
        .replace("{height}", &h.to_string())
        .replace(
            "{dest}",
            &format!("./{}", display_name(&delivery_dest(core, preset))),
        )
}

/// The Delivery section's chapter line (Roadmap Wave 2,
/// W2-18): when the project carries chapters an MP4
/// preset will embed, name the count before the press.
/// The markers ride every full MP4 export automatically
/// (the core embeds them — `export::embed_mp4_chapters`),
/// so this is a readout, not a control, and there is
/// deliberately no "chapters" preset chip: a preset is a
/// format × scale point, and a chip implying the other
/// presets lack markers would be a false distinction.
/// `None` when no chapter will embed — the section then
/// renders exactly as it did before. The count comes
/// from the core's own resolution
/// (`export::chapter_span_count`), so the line and the
/// export can never disagree. The wording is the
/// `export_delivery_chapters` catalogue template
/// (IA-W2-18-3) with the count as its slot.
pub fn delivery_chapters_text(core: &EditorCore) -> Option<String> {
    let count = stykanimate_rs::export::chapter_span_count(&core.project);
    if count == 0 {
        return None;
    }
    Some(
        i18n_wire::tr(core, "export_delivery_chapters")
            .replace("{count}", &count.to_string()),
    )
}

/// One format row's guidance note with its real destination
/// composed in (Roadmap Wave 1, RM23, IA-RM23-3): the six
/// converted `export_note_*` values carry a `{dest}` slot in
/// place of the legacy fixed path, and the row composes the
/// op's project-stem destination — the same string the op
/// itself writes to (`ExportOp::dest_named`).
fn note_with_dest(core: &EditorCore, key: &str, op: ExportOp) -> String {
    i18n_wire::tr(core, key)
        .replace("{dest}", &op.dest_named(&project_stem(&core.project)))
}

// ---- Publish package (Roadmap Wave 1, RM23) -----------
// P-13's bundle: the delivery MP4 (the selected MP4
// preset's leg — the GIF preset falls back to the
// master leg, a package carries an MP4 by
// definition) + the SRT and VTT sidecars from the
// subtitle writers + a thumbnail still of the
// playhead frame at 1280 wide. Members run in this
// order under the core bundle-progress contract,
// each allocated before the first runs (the MP4 its
// sampled frame total, each file member 1), and the
// recap names every member with its destination —
// the bundle recap's shapes, so the classifier reads
// a publish recap exactly like a bundle recap.

/// The publish package's member names, in run order —
/// the failure names the recap and the result carry.
pub const PUBLISH_MEMBERS: [&str; 4] = ["MP4", "SRT", "VTT", "thumbnail"];

/// The MP4 preset a publish package uses: the
/// selected preset when it is an MP4 preset, the
/// master when the GIF preset is selected.
pub fn publish_mp4_preset(preset: DeliveryPreset) -> DeliveryPreset {
    if preset.is_gif() || matches!(preset, DeliveryPreset::Shorts) {
        DeliveryPreset::Master
    } else {
        preset
    }
}

/// The publish package's four destination files,
/// under the export root, named from the project
/// stem and the package's MP4 preset.
pub fn publish_targets(core: &EditorCore, preset: DeliveryPreset) -> Vec<PathBuf> {
    let stem = project_stem(&core.project);
    let dir = export_dir();
    vec![
        dir.join(delivery_file_name(&stem, publish_mp4_preset(preset))),
        dir.join(format!("{stem}.srt")),
        dir.join(format!("{stem}.vtt")),
        dir.join(format!("{stem}-thumbnail.png")),
    ]
}

/// The recap line for a finished publish package,
/// built from the members that failed: every member
/// that ran and did not fail, with its destination,
/// in run order; the failures named after the
/// "; failed:" marker — `bundle_recap`'s exact
/// shapes, one member list over.
pub fn publish_recap(failed: &[&str], stem: &str, mp4_name: &str) -> String {
    let dests: [(&str, String); 4] = [
        ("MP4", format!("./export/{mp4_name}")),
        ("SRT", format!("./export/{stem}.srt")),
        ("VTT", format!("./export/{stem}.vtt")),
        ("thumbnail", format!("./export/{stem}-thumbnail.png")),
    ];
    let mut done: Vec<String> = Vec::new();
    for (name, dest) in &dests {
        if !failed.contains(name) {
            done.push(format!("{name} → {dest}"));
        }
    }
    let mut recap = if done.is_empty() {
        "Exported nothing".to_string()
    } else {
        format!("Exported {}", done.join(", "))
    };
    if !failed.is_empty() {
        recap += &format!("; failed: {}", failed.join(", "));
    }
    recap
}

/// Write the publish package's thumbnail member: the
/// playhead frame rendered at 1280 pt wide (the core
/// scaled still entry — a true re-render, not an
/// upscale) and saved as PNG. No frames is an error,
/// never an empty success.
fn write_thumbnail(
    project: &stykanimate_rs::model::Project,
    frame: usize,
    path: &Path,
) -> std::io::Result<()> {
    if project.frames.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the project has no frames to thumbnail",
        ));
    }
    let frame = frame.min(project.frames.len() - 1);
    let w = project.stage[0];
    let scale = if w.is_finite() && w > 0.0 {
        1280.0 / w
    } else {
        1.0
    };
    let img = stykanimate_rs::export::render_frame_scaled(project, frame, scale)?;
    img.save(path).map_err(std::io::Error::other)
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

    /// Where the format's full export writes when the
    /// destination is named from the project (Roadmap
    /// Wave 1, RM23 — P-13's named destinations): the
    /// single-file formats carry the project stem
    /// (`project_stem`) instead of the shared `animation`
    /// stem every episode used to overwrite; the
    /// sequence formats write into the same directories
    /// as ever (their frames are regenerable and named
    /// by frame number, not by episode). With the stem
    /// `"animation"` this is `dest()` byte for byte.
    pub fn dest_named(self, stem: &str) -> String {
        match self {
            ExportOp::Png => "./export/".to_string(),
            ExportOp::Jpeg => "out/jpeg/".to_string(),
            ExportOp::Svg => "./export/svg/".to_string(),
            ExportOp::Spritesheet => format!("./export/{stem}-spritesheet.png"),
            ExportOp::ContactSheet => format!("./export/{stem}-contact-sheet.png"),
            ExportOp::Gif => format!("./export/{stem}.gif"),
            ExportOp::Apng => format!("out/{stem}.apng"),
            ExportOp::Mp4 => format!("./export/{stem}.mp4"),
            ExportOp::Webm => format!("./export/{stem}.webm"),
        }
    }
}

/// The bare file name a single-file format writes under
/// its output root, named from the project stem
/// (RM23) — the join half of `ExportOp::dest_named`:
/// the panel builds real paths from the root plus
/// this name, and the recap strings from
/// `dest_named`, and the two can never disagree.
/// `None` for the sequence formats, which write a
/// directory of frame files, not one named file.
pub fn file_name_for(op: ExportOp, stem: &str) -> Option<String> {
    match op {
        ExportOp::Spritesheet => Some(format!("{stem}-spritesheet.png")),
        ExportOp::ContactSheet => Some(format!("{stem}-contact-sheet.png")),
        ExportOp::Gif => Some(format!("{stem}.gif")),
        ExportOp::Apng => Some(format!("{stem}.apng")),
        ExportOp::Mp4 => Some(format!("{stem}.mp4")),
        ExportOp::Webm => Some(format!("{stem}.webm")),
        ExportOp::Png | ExportOp::Jpeg | ExportOp::Svg => None,
    }
}

/// The file stem every named destination is built
/// from (RM23): the project name, lowercased, with
/// every run of non-alphanumeric characters collapsed
/// to one `-` and no leading/trailing dash
/// ("The Last Promise" → `the-last-promise`).
/// Underscores are kept. A name with no alphanumeric
/// characters at all falls back to the historical
/// stem, `animation` — the name the destinations
/// carried before they were named.
pub fn project_stem(project: &stykanimate_rs::model::Project) -> String {
    let mut stem = String::new();
    let mut pending_dash = false;
    for ch in project.name.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if pending_dash && !stem.is_empty() {
                stem.push('-');
            }
            pending_dash = false;
            stem.extend(ch.to_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if stem.is_empty() {
        "animation".to_string()
    } else {
        stem
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
/// runs (Wave Z, Z8). The arithmetic itself is the
/// core's one home (Wave AB, AB7):
/// `export::bundle_member_totals`, shared with the MCP
/// `export_bundle` driver — this wrapper only maps the
/// panel's `BundleKind` onto the core's. Pure: reads
/// the project only.
pub fn bundle_member_totals(
    kind: BundleKind,
    project: &stykanimate_rs::model::Project,
) -> Vec<usize> {
    stykanimate_rs::export::bundle_member_totals(
        match kind {
            BundleKind::Images => stykanimate_rs::export::BundleKind::Images,
            BundleKind::Video => stykanimate_rs::export::BundleKind::Video,
        },
        project,
    )
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
    let dest = op.dest_named(&project_stem(&s.core.project));
    note_single_dest(s, op, range, &dest);
}

/// `note_single` with the destination given: the
/// jobbed file formats recap the destination the job
/// actually wrote (its baked `job.dest`, named at
/// press time), not a recomputed one — the finish
/// path passes it, so the recap names the real file
/// even if the project was renamed mid-job.
fn note_single_dest(s: &mut State, op: ExportOp, range: Option<(usize, usize)>, dest: &str) {
    s.export_recap = if export_failed(&s.core) {
        failure_recap(&s.core.status, op.name())
    } else {
        match range {
            Some(range) => {
                single_recap_range(op.name(), dest, range, matches!(op, ExportOp::Webm))
            }
            None => single_recap(op.name(), dest),
        }
    };
}

/// The recap line for a finished bundle, built from the
/// plan: every op that ran and did not fail, with its
/// destination, in plan order; the failed ops (if any) are
/// named after a "; failed:" marker instead of being
/// claimed as exported. Destinations are the named ones
/// (RM23): built from the project stem the bundle ran
/// under.
pub fn bundle_recap(kind: BundleKind, failed: &[ExportOp], stem: &str) -> String {
    let mut done: Vec<String> = Vec::new();
    for op in plan_bundle(kind) {
        if !failed.contains(&op) {
            done.push(format!("{} → {}", op.name(), op.dest_named(stem)));
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

/// The fewest seconds between two recorded timing
/// samples (a phase's terminal event always records —
/// see [`PhaseTiming::record`]).
const PHASE_SAMPLE_MIN_INTERVAL_SECS: f64 = 0.25;

/// The most timing samples a slot keeps. At the
/// minimum interval that spans ~128 s of phase; past
/// it the annex decimates (keeps every second sample)
/// rather than dropping the phase's early history, so
/// the whole-phase rate keeps its full span (the
/// decimation is lossy about the middle, never about
/// the ends' neighbourhood — see `record`).
const PHASE_SAMPLE_MAX: usize = 512;

/// A phase slot's timing annex (Roadmap Wave 2,
/// W2-50): when each stored event arrived, and the
/// count it carried — the measured evidence the ETA
/// estimator (the phase-ETA section below) reads.
/// Times are slot-arrival times of contract events:
/// render events are delivered synchronously by the
/// render loop itself, encode events by the encode's
/// drainer thread as ffmpeg reports them, bundle
/// events by the core tracker — so an arrival time is
/// the event's production time plus at most a pipe's
/// lag, which is the honest basis the ETA line names
/// ("the rate measured so far"). The annex is
/// session-only by construction: it lives and dies
/// inside the running job's slots, and no rate is
/// ever carried between jobs — a rate measured on one
/// project says nothing honest about another.
#[derive(Default)]
struct PhaseTiming {
    /// The first stored event's arrival: every
    /// sample's elapsed is measured from it, so the
    /// first sample sits at 0.0 by construction.
    origin: Option<Instant>,
    /// (elapsed seconds, done) per recorded sample,
    /// oldest first, monotonic in both coordinates —
    /// every contract's stream is strictly advancing,
    /// and `record` refuses a regressive count rather
    /// than learn from one.
    samples: Vec<(f64, usize)>,
}

impl PhaseTiming {
    /// Record one stored event's arrival. Thinned to
    /// [`PHASE_SAMPLE_MIN_INTERVAL_SECS`] (the
    /// contracts can fire per frame) except the
    /// terminal event, which always records so the
    /// phase's measured span ends at its true end.
    fn record(&mut self, done: usize, finished: bool, now: Instant) {
        let origin = *self.origin.get_or_insert(now);
        let elapsed = now.duration_since(origin).as_secs_f64();
        if let Some(&(last_t, last_done)) = self.samples.last() {
            if done < last_done {
                return;
            }
            if !finished && elapsed - last_t < PHASE_SAMPLE_MIN_INTERVAL_SECS {
                return;
            }
        }
        if self.samples.len() >= PHASE_SAMPLE_MAX {
            self.samples = self.samples.iter().step_by(2).copied().collect();
        }
        self.samples.push((elapsed, done));
    }
}

/// One progress slot — a phase's, or a bundle job's
/// (Wave Z, Z8) — shared between the contract callback
/// (the only writer) and the UI thread (reader). The
/// callback copies each event's
/// fields verbatim — nothing is computed, estimated or
/// interpolated — so every number the panel shows is a
/// number the contract delivered. `seen` separates
/// "this phase has not reported yet" (the readout shows
/// no number for it) from a reported zero.
///
/// Roadmap Wave 2 (W2-50) adds the timing annex
/// (`timing`, [`PhaseTiming`]): the one companion fact
/// a delivered number gains is WHEN it arrived. The
/// snapshot below stays verbatim — the annex feeds
/// only the phase-ETA line, whose own section owns
/// the estimate wording and its honesty rules.
#[derive(Default)]
pub struct PhaseProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    finished: AtomicBool,
    seen: AtomicBool,
    timing: Mutex<PhaseTiming>,
}

impl PhaseProgress {
    fn store_render(&self, ev: stykanimate_rs::export::RenderProgressEvent) {
        self.store_render_at(ev, Instant::now());
    }

    /// `store_render` at an explicit arrival time —
    /// the seam the timing pins drive, so no pin ever
    /// sleeps or reads a wall clock for its evidence.
    fn store_render_at(
        &self,
        ev: stykanimate_rs::export::RenderProgressEvent,
        now: Instant,
    ) {
        self.done.store(ev.rendered_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        self.record_timing(ev.rendered_frames, ev.finished, now);
        // Written last: a reader that sees `seen` also
        // sees this event's numbers.
        self.seen.store(true, Ordering::SeqCst);
    }

    fn store_encode(&self, ev: stykanimate_rs::export::EncodeProgressEvent) {
        self.store_encode_at(ev, Instant::now());
    }

    /// `store_encode` at an explicit arrival time (see
    /// `store_render_at`).
    fn store_encode_at(
        &self,
        ev: stykanimate_rs::export::EncodeProgressEvent,
        now: Instant,
    ) {
        self.done.store(ev.encoded_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        self.record_timing(ev.encoded_frames, ev.finished, now);
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
        self.store_bundle_at(ev, Instant::now());
    }

    /// `store_bundle` at an explicit arrival time (see
    /// `store_render_at`).
    fn store_bundle_at(
        &self,
        ev: stykanimate_rs::export::BundleProgressEvent,
        now: Instant,
    ) {
        self.done.store(ev.completed_frames, Ordering::SeqCst);
        self.total.store(ev.total_frames, Ordering::SeqCst);
        self.finished.store(ev.finished, Ordering::SeqCst);
        self.record_timing(ev.completed_frames, ev.finished, now);
        self.seen.store(true, Ordering::SeqCst);
    }

    /// Feed the timing annex one stored event (W2-50).
    /// A poisoned annex lock drops the sample rather
    /// than fail the export — the callback discipline
    /// the contracts themselves promise their
    /// observers.
    fn record_timing(&self, done: usize, finished: bool, now: Instant) {
        if let Ok(mut timing) = self.timing.lock() {
            timing.record(done, finished, now);
        }
    }

    /// The slot's recorded timing samples, oldest
    /// first: (seconds since the phase's first stored
    /// event, frames done at that event). The ETA
    /// estimator's whole input, beside the snapshot's
    /// total.
    pub fn eta_samples(&self) -> Vec<(f64, usize)> {
        match self.timing.lock() {
            Ok(timing) => timing.samples.clone(),
            Err(_) => Vec::new(),
        }
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
    /// A delivery-preset MP4 (RM23): the full MP4 at
    /// the preset's scale (computed in the worker
    /// from the cloned project, like every other
    /// baked input), through the scaled entry (at
    /// 1.0 the entry itself routes to the unscaled
    /// funnel).
    Mp4Preset(DeliveryPreset),
    /// The publish package (RM23): a job whose worker
    /// runs the four package members, not one format.
    BundlePublish,
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
            JobKind::Mp4Full | JobKind::Mp4Range(..) | JobKind::Mp4Preset(_) => ExportOp::Mp4,
            JobKind::WebmFull | JobKind::WebmRange(..) => ExportOp::Webm,
            JobKind::BundleImages => ExportOp::Png,
            JobKind::BundleVideo => ExportOp::Mp4,
            JobKind::BundlePublish => ExportOp::Mp4,
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

    /// Whether this job runs the publish package
    /// (RM23) — its own branch in the finish and the
    /// readout, like a bundle's, but with its own
    /// member list and recap builder.
    fn publish(self) -> bool {
        matches!(self, JobKind::BundlePublish)
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
    /// A finished publish package (RM23): the members
    /// that failed, by name (`PUBLISH_MEMBERS`), in
    /// run order — the same data-not-error discipline
    /// as `Bundle`.
    Publish(Vec<&'static str>),
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
    // RM23: the video destinations are named from the
    // project stem at press time and baked into the
    // job, like the range.
    let stem = project_stem(&s.core.project);
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
            export_dir().join(file_name_for(ExportOp::Mp4, &stem).expect("MP4 is a file format")),
        ),
        (ExportOp::Mp4, None) => (
            JobKind::Mp4Full,
            export_dir().join(file_name_for(ExportOp::Mp4, &stem).expect("MP4 is a file format")),
        ),
        (ExportOp::Webm, Some((a, b))) => (
            JobKind::WebmRange(a, b),
            export_dir().join(file_name_for(ExportOp::Webm, &stem).expect("WebM is a file format")),
        ),
        (ExportOp::Webm, None) => (
            JobKind::WebmFull,
            export_dir().join(file_name_for(ExportOp::Webm, &stem).expect("WebM is a file format")),
        ),
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
        JobKind::Mp4Preset(preset) => {
            if matches!(preset, DeliveryPreset::Shorts) {
                // The Shorts reframe (W2-43): the core
                // entry owns the render scale and the
                // crop; the job's hooks ride along.
                return stykanimate_rs::export::export_mp4_shorts_with_progress(
                    project,
                    dest,
                    Some(&encode_cb),
                    Some(&render_cb),
                )
                .map(ExportJobResult::Mp4)
                .map_err(|e| e.to_string());
            }
            stykanimate_rs::export::export_mp4_scaled_with_progress(
                project,
                dest,
                delivery_scale(project, preset),
                Some(&encode_cb),
                Some(&render_cb),
            )
            .map(ExportJobResult::Mp4)
        }
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
        // (the plan-driven worker), and a publish job's
        // calls `run_publish_job`. Reaching this arm is
        // a dispatch bug, surfaced as a failed job rather
        // than a panic.
        JobKind::BundleImages | JobKind::BundleVideo | JobKind::BundlePublish => {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "bundle jobs run through run_bundle_job",
            ))
        }
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
    // The overwrite guard (RM23): the jobbed file
    // formats (MP4/WebM) name a finished master —
    // an existing one stops the press until it is
    // confirmed. The sequence formats write into a
    // directory and stay unguarded.
    if matches!(op, ExportOp::Mp4 | ExportOp::Webm) && guard_blocks(s, std::slice::from_ref(&dest)) {
        return false;
    }
    // The directory creation the MP4/WebM range actions
    // performed before calling (the sequence entries and
    // the encode funnels create their own parents).
    if matches!(kind, JobKind::Mp4Range(..) | JobKind::WebmRange(..)) {
        let _ = std::fs::create_dir_all(export_dir());
    }
    // RM29: a sequence job records what it is expected
    // to write, under the name Recent exports will give
    // its row, before the worker starts — a job that
    // dies mid-write is exactly the case the incomplete
    // mark exists for (`mark_incomplete`).
    match op {
        ExportOp::Png => note_expected_sequence(
            &display_name(&export_dir().join("frame-*.png")),
            expected_sequence_frames(&s.core.project, kind.range()),
        ),
        ExportOp::Jpeg => note_expected_sequence(
            &display_name(&PathBuf::from("out").join("jpeg")),
            expected_sequence_frames(&s.core.project, kind.range()),
        ),
        _ => {}
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
            let result = export::export_spritesheet_pack(
                project,
                &dest.join(
                    file_name_for(ExportOp::Spritesheet, &project_stem(project))
                        .expect("spritesheet is a file format"),
                ),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Spritesheet);
            }
        }
        BundleKind::Video => {
            // RM23: the members write the named
            // destinations, like the single-format jobs.
            let stem = project_stem(project);
            let member_cb = tracker.encode_callback(0);
            let result = export::export_mp4_with_encode_progress(
                project,
                &dest.join(
                    file_name_for(ExportOp::Mp4, &stem).expect("MP4 is a file format"),
                ),
                Some(&member_cb),
            );
            tracker.complete_member(result.is_ok());
            if result.is_err() {
                failed.push(ExportOp::Mp4);
            }
            let member_cb = tracker.encode_callback(1);
            let result = export::export_webm_with_encode_progress(
                project,
                &dest.join(
                    file_name_for(ExportOp::Webm, &stem).expect("WebM is a file format"),
                ),
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
    // The overwrite guard (RM23): a bundle whose file
    // members already exist stops at the first press,
    // like a single-format press.
    let targets = bundle_file_targets(kind, &s.core.project, &dest);
    if guard_blocks(s, &targets) {
        return false;
    }
    // RM29: the images bundle's two sequence members
    // record their expected counts like single sequence
    // jobs do (full exports — bundles ignore the range
    // toggle).
    if matches!(kind, BundleKind::Images) {
        note_expected_sequence(
            &display_name(&export_dir().join("frame-*.png")),
            expected_sequence_frames(&s.core.project, None),
        );
        note_expected_sequence(
            &display_name(&PathBuf::from("out").join("jpeg")),
            expected_sequence_frames(&s.core.project, None),
        );
    }
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

/// The publish package's worker body (RM23): run the
/// four members in order over the cloned project under
/// a core `BundleProgress` tracker allocated before
/// the first member runs — the MP4 member its sampled
/// frame total, each file member 1 — exactly the
/// bundle discipline. The MP4 member runs through the
/// (scaled) encode entry the delivery preset names,
/// with the tracker's encode translator as its
/// callback; the sidecars are written from the
/// subtitle writers' documents; the thumbnail is the
/// playhead frame at 1280 wide. Every member is
/// attempted — a failure is recorded by name and the
/// package continues, as a bundle does — and the
/// tracker is finished after the last member. Runs
/// entirely off the UI thread; touches no shell state.
fn run_publish_job(
    project: &stykanimate_rs::model::Project,
    dest: &Path,
    stem: &str,
    mp4_name: &str,
    scale: f32,
    frame: usize,
    bundle: &PhaseProgress,
) -> ExportJobOutcome {
    use stykanimate_rs::export;
    // The sidecar writes do not create their parent
    // (the MP4 funnel does, but it may have failed
    // before reaching its own creation) — the package
    // root is created up front, like the actions do.
    let _ = std::fs::create_dir_all(dest);
    let bundle_cb = |ev: export::BundleProgressEvent| bundle.store_bundle(ev);
    let totals = [export::sampled_frame_total(project), 1, 1, 1];
    let tracker = export::BundleProgress::new(&totals, Some(&bundle_cb));
    let mut failed: Vec<&'static str> = Vec::new();
    let member_cb = tracker.encode_callback(0);
    let result = if scale == 1.0 {
        export::export_mp4_with_encode_progress(project, &dest.join(mp4_name), Some(&member_cb))
    } else {
        export::export_mp4_scaled_with_encode_progress(
            project,
            &dest.join(mp4_name),
            scale,
            Some(&member_cb),
        )
    };
    tracker.complete_member(result.is_ok());
    if result.is_err() {
        failed.push("MP4");
    }
    let srt = stykanimate_rs::subtitles::subtitles_to_srt(project);
    let ok = std::fs::write(dest.join(format!("{stem}.srt")), srt).is_ok();
    tracker.complete_member(ok);
    if !ok {
        failed.push("SRT");
    }
    let vtt = stykanimate_rs::subtitles::subtitles_to_vtt(project);
    let ok = std::fs::write(dest.join(format!("{stem}.vtt")), vtt).is_ok();
    tracker.complete_member(ok);
    if !ok {
        failed.push("VTT");
    }
    let ok = write_thumbnail(project, frame, &dest.join(format!("{stem}-thumbnail.png"))).is_ok();
    tracker.complete_member(ok);
    if !ok {
        failed.push("thumbnail");
    }
    tracker.finish();
    Ok(ExportJobResult::Publish(failed))
}

/// Begin the background job for the publish package —
/// the body of the Publish package row's press.
/// Returns false, changing nothing but the guard's
/// record, when a job is already active (one export
/// at a time) or when the overwrite guard stops the
/// press (any of the four destination files exists
/// and this is not the confirming second press).
fn start_publish_job(s: &mut State) -> bool {
    if s.export_job.is_some() {
        return false;
    }
    let preset = delivery_preset();
    let targets = publish_targets(&s.core, preset);
    if guard_blocks(s, &targets) {
        return false;
    }
    let mp4_preset = publish_mp4_preset(preset);
    let stem = project_stem(&s.core.project);
    let mp4_name = delivery_file_name(&stem, mp4_preset);
    let scale = delivery_scale(&s.core.project, mp4_preset);
    let frame = s.core.current;
    let dest = export_dir();
    let work_dest = dest.clone();
    let project = s.core.project.clone();
    begin_job(
        s,
        JobKind::BundlePublish,
        dest,
        project,
        Box::new(move |project, _render, _encode, bundle| {
            run_publish_job(project, &work_dest, &stem, &mp4_name, scale, frame, bundle)
        }),
    );
    true
}

/// Begin the background job for an MP4 delivery
/// preset (the Export delivery press on an MP4
/// preset): the full MP4 through the scaled entry at
/// the preset's scale, to the preset's named
/// destination, under the same one-at-a-time and
/// overwrite-guard discipline as a single-format job.
fn start_delivery_job(s: &mut State, preset: DeliveryPreset) -> bool {
    if s.export_job.is_some() {
        return false;
    }
    let dest = delivery_dest(&s.core, preset);
    if guard_blocks(s, std::slice::from_ref(&dest)) {
        return false;
    }
    let kind = JobKind::Mp4Preset(preset);
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

/// The frame-tick drain for the export job, called from
/// `main::poll`. Returns `(redraw, active)`: redraw is
/// asked only when a phase snapshot changed since the
/// last shown one (or the job just finished and its
/// status/recap landed); active is true while the job
/// runs, so the caller can keep the wake cadence
/// (`JOB_WAKE`) alive. A worker that dies without
/// sending (a panic in the export) finishes the job as
/// a failure in the op's voice — never as a success and
/// never silently dropped. W2-44: a landed QA probe
/// report also asks for a redraw, and a probe in
/// flight also reads active (see `poll_qa_probes`).
pub fn poll_export_job(s: &mut State) -> (bool, bool) {
    // The probe side is drained first; a probe queued
    // by this very tick's finish (below) is picked up
    // by the active check and drained on the next
    // tick — the worker needs the time anyway.
    let qa_redraw = poll_qa_probes();
    let (redraw, active) = poll_export_job_drain(s);
    (redraw || qa_redraw, active || qa_probe_active())
}

/// The export-job half of the frame-tick drain (see
/// `poll_export_job` for the whole contract).
fn poll_export_job_drain(s: &mut State) -> (bool, bool) {
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
    // A publish package job (RM23) finishes the way a
    // bundle job does — its recap, built from the
    // members that failed, is the status and the panel
    // recap together. The stem and the MP4 member's
    // name are recomputed from the same state the
    // press read (the preset chips are inert while a
    // job runs, so the selection cannot have moved).
    // A job-level `Err` means no member outcome was
    // reported, so every member counts as failed.
    if job.kind.publish() {
        let failed: Vec<&'static str> = match result {
            Ok(ExportJobResult::Publish(failed)) => failed,
            _ => PUBLISH_MEMBERS.to_vec(),
        };
        let stem = project_stem(&s.core.project);
        let mp4_name = delivery_file_name(&stem, publish_mp4_preset(delivery_preset()));
        let recap = publish_recap(&failed, &stem, &mp4_name);
        s.core.status = recap.clone();
        s.export_recap = recap;
        // RM29: probe the package's MP4 leg like any
        // finished video export — its own QA record
        // beside the recap, which is not rewritten.
        clear_residue();
        if !failed.iter().any(|f| *f == "MP4") {
            let dest = job.dest.join(&mp4_name);
            queue_probe(&s.core.project, &dest, None, true);
        }
        return;
    }
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
        let stem = project_stem(&s.core.project);
        let recap = bundle_recap(kind, &failed, &stem);
        s.core.status = recap.clone();
        s.export_recap = recap;
        // RM29: the video bundle's members are finished
        // video exports — probe each one that did not
        // fail, the WebM under its video-only audio rule.
        clear_residue();
        if matches!(kind, BundleKind::Video) {
            for (member, audio_possible) in [(ExportOp::Mp4, true), (ExportOp::Webm, false)] {
                if !failed.contains(&member) {
                    let dest = job
                        .dest
                        .join(file_name_for(member, &stem).expect("video member is a file format"));
                    queue_probe(&s.core.project, &dest, None, audio_possible);
                }
            }
        }
        return;
    }
    let op = job.kind.op();
    // RM29: a failed sequence job states its residue —
    // the frame files the attempt left in the
    // destination directory — on the residue line. The
    // status below keeps the op's own failure sentence
    // byte for byte; the residue is a second record.
    let job_failed = result.is_err();
    if job_failed && matches!(op, ExportOp::Png | ExportOp::Jpeg) {
        let left = sequence_residue(&job.dest);
        if left > 0 {
            store_residue(left, &job.dest);
        }
    }
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
            (JobKind::Mp4Full | JobKind::Mp4Preset(_), ExportJobResult::Mp4(outcome)) => {
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
    // RM23: the jobbed file formats recap the
    // destination the job actually wrote (named at
    // press time and baked into the job), in the
    // `./export/…` display shape the recaps have
    // always used; the sequence formats recap their
    // directory through `note_single` as before.
    match op {
        ExportOp::Mp4 | ExportOp::Webm => {
            let dest = format!("./{}", display_name(&job.dest));
            note_single_dest(s, op, job.kind.range(), &dest);
        }
        _ => note_single(s, op, job.kind.range()),
    }
    // RM29: on success the residue of any earlier failed
    // sequence is stale — clear it — and a finished
    // MP4/WebM is probed: the QA report lands as its
    // own record beside the recap (never a rewrite of
    // the status or the recap above). W2-44: the probe
    // is queued onto a worker here; its report is
    // stored when the frame tick drains it. The full
    // WebM is the one path whose audio is not expected.
    if !job_failed {
        clear_residue();
        if matches!(op, ExportOp::Mp4 | ExportOp::Webm) {
            let audio_possible = !matches!(job.kind, JobKind::WebmFull);
            queue_probe(&s.core.project, &job.dest, job.kind.range(), audio_possible);
        }
    }
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
///
/// Roadmap Wave 2 (W2-50) appends one more line when
/// a phase is actively reporting: the phase ETA
/// (`eta_line`, the section below) — the readout's
/// only computed number, worded as the estimate it
/// is, with its measured basis named in the line.
pub fn job_lines(core: &EditorCore, job: &ExportJob) -> Vec<(&'static str, String)> {
    if let Some(kind) = job.kind.bundle_kind() {
        let bundle = job.bundle.snapshot();
        let mut lines = if bundle.seen {
            vec![("xc-progress-bundle", bundle_progress_line(core, bundle))]
        } else {
            vec![(
                "xc-progress-working",
                i18n_wire::tr(core, "export_progress_working").replace("{name}", kind.name()),
            )]
        };
        if let Some(eta) = eta_line(core, job) {
            lines.push(eta);
        }
        return lines;
    }
    if job.kind.publish() {
        // A publish package (RM23) reads its bundle
        // slot exactly like a bundle job — its members
        // report through the one bundle contract — with
        // the package's own name on the working line
        // (slot data, like `BundleKind::name()`).
        let bundle = job.bundle.snapshot();
        let mut lines = if bundle.seen {
            vec![("xc-progress-bundle", bundle_progress_line(core, bundle))]
        } else {
            vec![(
                "xc-progress-working",
                i18n_wire::tr(core, "export_progress_working")
                    .replace("{name}", "publish package"),
            )]
        };
        if let Some(eta) = eta_line(core, job) {
            lines.push(eta);
        }
        return lines;
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
    if let Some(eta) = eta_line(core, job) {
        lines.push(eta);
    }
    lines
}

// ---- Phase ETAs (Roadmap Wave 2, W2-50) ----
// Per-phase remaining-time estimates for the running
// job's readout. The phase lines above report each
// phase's own counts and never merge them — rightly:
// the phases' costs differ wildly, so a single
// percentage would lie by omission early and late.
// What a long export's animator actually waits on is
// time, and time is what this section estimates —
// per phase, from that phase's own measured
// throughput (the timing annex on each slot: when
// each contract event arrived and the count it
// carried), under three honesty rules:
//
// - An estimate is quoted only from measured
//   evidence: a span of at least `ETA_MIN_SPAN_SECS`
//   seconds across which at least `ETA_MIN_ADVANCE`
//   frames advanced. Before that the line says the
//   estimate is still forming ("estimating…") — it
//   never fills the gap with a guess dressed as a
//   number.
// - When the recent window's rate and the whole
//   phase's rate disagree by more than
//   `ETA_BRACKET_RATIO`, both ends are shown: the
//   bracket IS the two measured rates' remaining
//   times, not a padding invented around one of
//   them. Displayed times are quantised to 5-second
//   steps, bracket ends rounded outward, so the
//   display never claims second precision or a
//   narrower bracket than measured.
// - The claim is scoped to its phase in the line's
//   own wording, exactly as the encode line's
//   finished sentence claims only the encode: a video
//   job in its render phase says the encoding still
//   follows unestimated, and phases the pipeline
//   runs with no contract at all (an MP4's audio
//   mux, the chapter embed) are never estimated —
//   see `EtaPhase`.
//
// The estimate is a pure function of the annex's
// samples and the snapshot's total, so the pins
// below drive it with synthetic sequences at
// injected times; nothing here sleeps, and no rate
// survives the job that measured it.

/// One measured phase of an export job, as the ETA
/// line names it. The set is closed and small
/// because the progress contracts are: a job's frames
/// are counted by exactly three contracts (render,
/// encode, bundle), each owning exactly one slot on
/// `ExportJob`. Steps the pipeline runs WITHOUT a
/// contract — an MP4's audio mux and chapter embed,
/// after the encode terminal — are deliberately
/// absent: there is no measured rate to estimate
/// them from, and inventing one would be fake
/// precision. The ETA line's claim is scoped to its
/// phase by its wording instead.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EtaPhase {
    Render,
    Encode,
    Bundle,
}

impl EtaPhase {
    /// Every phase the readout can estimate, in
    /// pipeline order.
    pub const ALL: [EtaPhase; 3] =
        [EtaPhase::Render, EtaPhase::Encode, EtaPhase::Bundle];
}

/// The phases a job runs, in pipeline order — the
/// readout's census of the pipeline's measured
/// stages, per job shape: sequence exports measure
/// only their render loop; video exports measure
/// render then encode (their mux / chapter-embed
/// tail is unmeasured — see `EtaPhase`); bundle-
/// shaped jobs measure the one bundle contract.
/// Pinned per shape by
/// `job_phases_match_the_pipelines_measured_stages`.
pub fn job_phases(kind: JobKind) -> &'static [EtaPhase] {
    match kind {
        JobKind::PngFull | JobKind::PngRange(..) | JobKind::JpegFull | JobKind::JpegRange(..) => {
            &[EtaPhase::Render]
        }
        JobKind::Mp4Full
        | JobKind::Mp4Range(..)
        | JobKind::WebmFull
        | JobKind::WebmRange(..)
        | JobKind::Mp4Preset(_) => &[EtaPhase::Render, EtaPhase::Encode],
        JobKind::BundleImages | JobKind::BundleVideo | JobKind::BundlePublish => {
            &[EtaPhase::Bundle]
        }
    }
}

/// The slot a phase's contract writes — the phase /
/// slot / event-type correspondence the census pin
/// holds (`job_phases_match_the_pipelines_measured_stages`).
fn phase_slot(job: &ExportJob, phase: EtaPhase) -> &PhaseProgress {
    match phase {
        EtaPhase::Render => &job.render,
        EtaPhase::Encode => &job.encode,
        EtaPhase::Bundle => &job.bundle,
    }
}

/// The phase currently running, if any: the LAST
/// phase in the job's table whose slot has reported
/// and not finished (the phases are sequential, so a
/// later phase reporting means the earlier ones are
/// done). Between phases — render finished, encode
/// not yet reported — and after the last measured
/// phase finishes (the unmeasured mux tail), there
/// is no active phase and the readout shows no ETA
/// line: the gap is stated by the line's absence,
/// not papered over with a stale estimate.
fn active_phase(job: &ExportJob) -> Option<EtaPhase> {
    job_phases(job.kind).iter().rev().copied().find(|&phase| {
        let snap = phase_slot(job, phase).snapshot();
        snap.seen && !snap.finished
    })
}

/// The shortest measured span (seconds of delivered
/// events) a rate may be computed over. Below it, a
/// rate is two timestamps and a hope.
pub const ETA_MIN_SPAN_SECS: f64 = 2.0;

/// The fewest frames that must have advanced across
/// a measured span before a rate is quoted — one or
/// two frames of evidence would swing the estimate
/// by whole minutes either way.
pub const ETA_MIN_ADVANCE: usize = 4;

/// The trailing window (seconds of delivered events)
/// the recent rate is measured over, when the phase
/// has run that long.
pub const ETA_WINDOW_SECS: f64 = 10.0;

/// How far apart the recent and whole-phase
/// remaining estimates may be (as a ratio) before
/// both ends are shown instead of one number.
pub const ETA_BRACKET_RATIO: f64 = 1.25;

/// What the estimator can honestly say about a
/// phase's remaining time.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EtaEstimate {
    /// No quotable rate yet: too few samples, too
    /// short a span, too little advance — or no work
    /// left to estimate. The line says the estimate
    /// is still forming; it never invents a number.
    Estimating,
    /// Remaining time derived from measured rates:
    /// `point_secs` is the single number quoted when
    /// the rates agree (the recent window's estimate
    /// when there is one — the freshest evidence);
    /// `low_secs` / `high_secs` are the two rates'
    /// remaining times, and `bracketed` says they
    /// disagreed enough to show both.
    Remaining {
        point_secs: f64,
        low_secs: f64,
        high_secs: f64,
        bracketed: bool,
    },
}

/// A rate (frames per second) over one sample span,
/// when the span is long enough and advanced enough
/// to quote. A regressive or frozen span yields no
/// rate rather than a division by ~zero dressed as
/// an estimate.
fn eta_span_rate(from: (f64, usize), to: (f64, usize)) -> Option<f64> {
    let dt = to.0 - from.0;
    let dd = to.1.checked_sub(from.1)?;
    if dt >= ETA_MIN_SPAN_SECS && dd >= ETA_MIN_ADVANCE {
        Some(dd as f64 / dt)
    } else {
        None
    }
}

/// Estimate a phase's remaining time from its timing
/// samples (oldest first: seconds since the phase's
/// first stored event, frames done) and its total.
/// Pure: the pins drive it with synthetic sequences
/// at injected times.
pub fn estimate_eta(samples: &[(f64, usize)], total: usize) -> EtaEstimate {
    let Some(&last) = samples.last() else {
        return EtaEstimate::Estimating;
    };
    if total == 0 || last.1 >= total {
        // Nothing left to estimate (the terminal is
        // in flight; the outcome replaces the readout
        // on the same tick).
        return EtaEstimate::Estimating;
    }
    let remaining = (total - last.1) as f64;
    let overall = eta_span_rate(samples[0], last);
    // The recent window's rate: from the first sample
    // inside the trailing window to the last. When
    // the whole span fits the window this IS the
    // overall rate (the same two points), and when
    // the window holds only the last sample the span
    // is a point and yields no rate.
    let window_start = last.0 - ETA_WINDOW_SECS;
    let recent = samples
        .iter()
        .position(|s| s.0 >= window_start)
        .and_then(|i| eta_span_rate(samples[i], last));
    match (overall, recent) {
        (Some(all), Some(win)) => {
            let rem_all = remaining / all;
            let rem_win = remaining / win;
            let low = rem_all.min(rem_win);
            let high = rem_all.max(rem_win);
            EtaEstimate::Remaining {
                point_secs: rem_win,
                low_secs: low,
                high_secs: high,
                bracketed: high > low * ETA_BRACKET_RATIO,
            }
        }
        (Some(rate), None) | (None, Some(rate)) => {
            let rem = remaining / rate;
            EtaEstimate::Remaining {
                point_secs: rem,
                low_secs: rem,
                high_secs: rem,
                bracketed: false,
            }
        }
        (None, None) => EtaEstimate::Estimating,
    }
}

/// How a displayed ETA value rounds to the 5-second
/// display quantum: a bracket's ends round OUTWARD
/// (low down, high up) so the shown bracket is never
/// narrower than the measured one; a point rounds to
/// nearest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EtaRound {
    Down,
    Nearest,
    Up,
}

/// Seconds → the readout's time text: quantised to
/// whole 5-second steps (an estimate never displays
/// second precision it does not have), formatted
/// m:ss — the stats view's duration shape. A
/// positive value never displays as 0:00.
fn eta_time_text(secs: f64, round: EtaRound) -> String {
    let steps = secs / 5.0;
    let steps = match round {
        EtaRound::Down => steps.floor(),
        EtaRound::Nearest => steps.round(),
        EtaRound::Up => steps.ceil(),
    };
    let mut total = (steps.max(0.0) * 5.0) as u64;
    if secs > 0.0 && total == 0 {
        total = 5;
    }
    format!("{}:{:02}", total / 60, total % 60)
}

/// The name the ETA line gives its phase. Render and
/// encode take catalogue labels; the bundle slot
/// takes the same slot-data name the working line
/// interpolates (`BundleKind::name()`, the publish
/// package's standing literal) — data, not a label
/// site, exactly as in `job_lines`.
fn eta_phase_name(core: &EditorCore, job: &ExportJob, phase: EtaPhase) -> String {
    match phase {
        EtaPhase::Render => i18n_wire::tr(core, "export_phase_render"),
        EtaPhase::Encode => i18n_wire::tr(core, "export_phase_encode"),
        EtaPhase::Bundle => {
            if job.kind.publish() {
                "publish package".to_string()
            } else if let Some(kind) = job.kind.bundle_kind() {
                kind.name().to_string()
            } else {
                "bundle".to_string()
            }
        }
    }
}

/// The running job's ETA line, with its element key
/// — or nothing, when no phase is actively reporting
/// (before the first event, between phases, in the
/// unmeasured tail, or with no work left). The
/// line's wording scopes its claim to the phase and
/// names the measured basis; a video job still in
/// its render phase additionally names the encoding
/// that follows unestimated, so the render figure is
/// not read as the job's total.
fn eta_line(core: &EditorCore, job: &ExportJob) -> Option<(&'static str, String)> {
    let phase = active_phase(job)?;
    let slot = phase_slot(job, phase);
    let snap = slot.snapshot();
    if !snap.seen || snap.finished || snap.total == 0 || snap.done >= snap.total {
        return None;
    }
    let phase_name = eta_phase_name(core, job, phase);
    let mut line = match estimate_eta(&slot.eta_samples(), snap.total) {
        EtaEstimate::Estimating => i18n_wire::tr(core, "export_eta_estimating")
            .replace("{phase}", &phase_name),
        EtaEstimate::Remaining {
            point_secs,
            low_secs,
            high_secs,
            bracketed,
        } => {
            let low = eta_time_text(low_secs, EtaRound::Down);
            let high = eta_time_text(high_secs, EtaRound::Up);
            if bracketed && low != high {
                i18n_wire::tr(core, "export_eta_range")
                    .replace("{phase}", &phase_name)
                    .replace("{low}", &low)
                    .replace("{high}", &high)
            } else {
                i18n_wire::tr(core, "export_eta_about")
                    .replace("{phase}", &phase_name)
                    .replace("{time}", &eta_time_text(point_secs, EtaRound::Nearest))
            }
        }
    };
    if phase == EtaPhase::Render && job_phases(job.kind).contains(&EtaPhase::Encode) {
        line.push(' ');
        line.push_str(&i18n_wire::tr(core, "export_eta_then_encode"));
    }
    Some(("xc-progress-eta", line))
}

// ---- Cancel chrome (Roadmap Wave 2, W2-64 — PL-11) ----
// The StopSignal-ready half of the feedback surface.
// AC8 designed the core stop signal (a clonable flag
// the funnels poll at named observation points) as a
// DESIGN ONLY seat, and decision D-4 leaves the build
// — plus the class-C recap sentence for a stopped
// bundle and the readout rule between press and
// effect (AC8's Q1/Q2) — to a user ruling that has
// not been taken. Until it is, NO stop signal reaches
// any export worker: dropping `State::export_job`
// stops nothing (the worker owns its project clone
// and runs to completion), and the progress contracts
// are observe-only by type, so no honest live Cancel
// can exist. What this section builds is the chrome
// the signal will plug into, in its disabled-honest
// form: the seam below is the single question the
// panel asks, and `cancel_row` is the single place
// that renders the answer.

/// Can the running job be stopped from the panel?
/// The panel asks this one question per job; the
/// stop build (D-4 ruled, AC8's design) answers it
/// differently by adding the armed case here — at
/// which point `cancel_row`'s exhaustive match stops
/// compiling until the armed case gets its live
/// button, so the chrome and the signal cannot drift
/// apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobCancel {
    /// No stop signal reaches the worker: the job
    /// runs to completion whatever the user does.
    /// The affordance renders inert with its reason
    /// stated beside it — never as a live-looking
    /// button that does nothing.
    Unavailable,
}

/// The cancel question for the running job (W2-64).
/// Today the answer is `Unavailable` for every job
/// shape — single-format, delivery, bundle and
/// publish alike: `ExportJob` holds no signal clone
/// because the core signal does not exist yet (AC8
/// §2's `StopSignal` is a design, D-4 gates its
/// build). When it lands, the job will carry the
/// clone AC8 §6 prescribes and this function returns
/// the armed case for it.
pub fn job_cancel(_job: &ExportJob) -> JobCancel {
    JobCancel::Unavailable
}

/// The running job's cancel row: the affordance
/// itself plus, in the unavailable state, the reason
/// as a visible line beside it (a panel-local note —
/// never written to `core.status`, so no class-C
/// surface carries it). Mounted under the progress
/// lines for every job shape, so the surface is the
/// same whether the job is a single format, a
/// delivery preset, a bundle or the publish package.
pub fn cancel_row(core: &EditorCore, job: &ExportJob, t: &Theme) -> El {
    match job_cancel(job) {
        JobCancel::Unavailable => row()
            .gap(8.0)
            .center_y()
            .child(btn_inert(
                "xc-cancel",
                &i18n_wire::tr(core, "export_cancel"),
                t,
            ))
            .child(
                text(i18n_wire::tr(core, "export_cancel_unavailable"))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .grow(1.0),
            ),
    }
}

// ---- Post-export QA probe (Roadmap Wave 1, RM29 / P-16) ----
// Detectors only. Everything in this section READS a
// produced file and reports; nothing here writes an
// export, touches `core.status` or the recap, or alters
// a class-C sentence. The verdict is computed by the
// pure `qa_verdict` from plain measurements, so the
// whole decision matrix is pinnable without a file; the
// shell-outs (`ffprobe`, two bounded `ffmpeg` decodes)
// only gather measurements. Probe machinery is shared
// in spirit with RM1's F-1 family in `export.rs` — the
// same system ffmpeg/ffprobe the exporters themselves
// shell out to; the lib's own probe helpers are private
// to that module, so the panel carries its own thin
// readers here rather than widening the lib surface.




/// How many frames the black-frame scan samples across
/// the file's duration.
pub const QA_BLACK_SAMPLES: usize = 8;










/// Decode a produced file's audio with the system
/// `ffmpeg` (mono WAV at the mix rate) and meter it —
/// the measurement behind the silent-audio check. `None`
/// when there is nothing decodable (no audio stream,
/// ffmpeg missing): the verdict reads `None` beside an
/// expected-audio export as its own failure, and beside
/// a video-only export as no measurement.
pub fn decode_audio_stats(path: &Path) -> Option<(f32, f32)> {
    let tmp = qa_temp_path("audio", "wav");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", "44100"])
        .arg(&tmp)
        .status()
        .ok()?;
    let stats = if status.success() {
        stykanimate_rs::audio_edit::read_wav(&tmp.to_string_lossy())
            .ok()
            .map(|wav| audio_stats(&wav.samples))
    } else {
        None
    };
    let _ = std::fs::remove_file(&tmp);
    stats
}

/// Mean luma (0..=1) of one frame of a produced file,
/// decoded small and gray — one black-scan sample.
fn frame_luma_at(path: &Path, at_secs: f32) -> Option<f32> {
    let tmp = qa_temp_path("luma", "gray");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-ss"])
        .arg(format!("{at_secs:.3}"))
        .arg("-i")
        .arg(path)
        .args(["-frames:v", "1", "-vf", "scale=64:36", "-f", "rawvideo", "-pix_fmt", "gray"])
        .arg(&tmp)
        .status()
        .ok()?;
    let bytes = if status.success() {
        std::fs::read(&tmp).ok()
    } else {
        None
    };
    let _ = std::fs::remove_file(&tmp);
    let bytes = bytes?;
    if bytes.is_empty() {
        return None;
    }
    let sum: u64 = bytes.iter().map(|b| *b as u64).sum();
    Some(sum as f32 / bytes.len() as f32 / 255.0)
}

/// Sample `QA_BLACK_SAMPLES` frames across the file
/// for the black-frame scan. Undecodable samples are
/// skipped, not counted black — an empty result means
/// the scan itself found nothing to read, which the
/// verdict treats as no evidence (the stream checks
/// already passed or failed on their own).
pub fn black_frame_lumas(path: &Path, duration_secs: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for i in 0..QA_BLACK_SAMPLES {
        let t = duration_secs * (i as f32 + 0.5) / QA_BLACK_SAMPLES as f32;
        if let Some(luma) = frame_luma_at(path, t) {
            out.push(luma);
        }
    }
    out
}



/// Everything the probe measured about one file,
/// before the verdict: the verdict is a pure function
/// of this plus the expectations, so the decision
/// matrix needs no files at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QaMeasurements {
    pub probe: Option<VideoProbe>,
    pub audio: Option<(f32, f32)>,
    pub lumas: Vec<f32>,
    pub ink: Option<usize>,
}

/// One probed export's verdict: where it was probed,
/// whether it passed, the named failures (empty when
/// passed), and the measured-facts string the panel
/// line carries as its details slot.
#[derive(Clone, Debug, PartialEq)]
pub struct QaReport {
    pub dest: String,
    pub passed: bool,
    pub failures: Vec<String>,
    pub details: String,
}

/// The QA verdict — pure. Every check names its own
/// failure; a file that cannot even be probed fails
/// with that single fact rather than a cascade.
pub fn qa_verdict(dest: &str, expect: &QaExpectations, m: &QaMeasurements) -> QaReport {
    let Some(probe) = &m.probe else {
        return QaReport {
            dest: dest.to_string(),
            passed: false,
            failures: vec!["the produced file could not be probed".to_string()],
            details: "unreadable".to_string(),
        };
    };
    let mut failures: Vec<String> = Vec::new();
    if !probe.has_video {
        failures.push("no video stream".to_string());
    }
    let duration_tol = (expect.duration_secs * 0.15).max(0.75);
    if (probe.duration_secs - expect.duration_secs).abs() > duration_tol {
        failures.push(format!(
            "duration {:.1}s, expected {:.1}s",
            probe.duration_secs, expect.duration_secs
        ));
    }
    if let (Some(got), Some(want)) = (probe.frames, expect.frames) {
        let tol = ((want as f32 * 0.05).ceil() as u64).max(2);
        if got.abs_diff(want as u64) > tol {
            failures.push(format!("{got} frames, expected {want}"));
        }
    }
    if expect.audio_expected {
        if !probe.has_audio {
            failures.push("audio stream missing".to_string());
        } else {
            match m.audio {
                None => failures.push("audio could not be decoded".to_string()),
                Some((peak, _)) if peak <= QA_SILENCE_PEAK => {
                    failures.push(format!("audio is silent (peak {})", fmt_db(peak)))
                }
                _ => {}
            }
        }
    }
    if !m.lumas.is_empty() && m.lumas.iter().all(|l| *l <= QA_BLACK_LUMA) {
        failures.push(format!("all {} sampled frames are black", m.lumas.len()));
    }
    if expect.burn_in.is_some() {
        match m.ink {
            Some(ink) if ink >= QA_INK_MIN => {}
            Some(_) => failures.push("subtitle burn-in missing at the cue".to_string()),
            None => failures.push("subtitle burn-in could not be checked".to_string()),
        }
    }
    let frames_fact = probe
        .frames
        .map(|f| f.to_string())
        .unwrap_or_else(|| "?".to_string());
    let audio_fact = match m.audio {
        Some((peak, rms)) => format!("peak {} · RMS {}", fmt_db(peak), fmt_db(rms)),
        None => "no audio measured".to_string(),
    };
    let black = m.lumas.iter().filter(|l| **l <= QA_BLACK_LUMA).count();
    let burn_fact = match (&expect.burn_in, m.ink) {
        (None, _) => "no subtitles",
        (Some(_), Some(ink)) => {
            if ink >= QA_INK_MIN {
                "burn-in present"
            } else {
                "burn-in missing"
            }
        }
        (Some(_), None) => "burn-in unchecked",
    };
    let details = format!(
        "{:.1}s · {} frames · {} · black {}/{} · {}",
        probe.duration_secs,
        frames_fact,
        audio_fact,
        black,
        m.lumas.len(),
        burn_fact
    );
    QaReport {
        dest: dest.to_string(),
        passed: failures.is_empty(),
        failures,
        details,
    }
}

/// Probe one produced file end to end: ffprobe, the
/// audio decode (only when a stream is present), the
/// black-frame scan, and the burn-in spot-check when a
/// cue is expected. Gathering is best-effort per
/// measurement; the verdict decides what absence means.
pub fn probe_export(path: &Path, expect: &QaExpectations) -> QaReport {
    let probe = probe_video_file(path);
    let mut m = QaMeasurements {
        probe: probe.clone(),
        ..Default::default()
    };
    if let Some(p) = &probe {
        if p.has_audio {
            m.audio = decode_audio_stats(path);
        }
        if p.has_video {
            m.lumas = black_frame_lumas(path, p.duration_secs);
            if let Some(burn) = &expect.burn_in {
                m.ink = burn_in_ink(path, burn, (p.width, p.height));
            }
        }
    }
    qa_verdict(&format!("./{}", display_name(path)), expect, &m)
}

// ---- Episode QA probe (Roadmap Wave 2, W2-20 / P-16) ----
// The episode layer over RM29's probe: the same file,
// read against the whole project rather than one export
// window. RM29 answers "did this export survive" (a
// stream exists, the mix is not silent, one cue burned
// in); this layer answers the questions an episode QA
// pass asks — silent STRETCHES inside an otherwise
// sounding mix, black and frozen stretches, every
// chapter's title card, the MP4's chapter markers
// against the markers the export promised (W2-18), and
// the cues' own timing sanity against the timeline.
// Nothing here forks RM29: expectations ride
// `qa_expectations`, the stream probe is
// `probe_video_file`, the cue spot-check is
// `burn_in_ink`, and the metering is `audio_stats` over
// the same ffmpeg decode. What is new is the stretch /
// marker / card measurement family and a report of
// NAMED checks, each Pass / Fail / Unmeasured with its
// evidence — Unmeasured is a first-class outcome, never
// a silent pass: a check that could not measure says so.
// Threshold discipline (roadmap D-9): every threshold
// is a named parameter in `EpisodeQaParams` with its
// default stated, and the subtitle reading-speed figure
// is REPORTED against its reference parameter, never
// judged — the roadmap has not set that threshold.










































/// The panel line for one report: the pass/fail
/// template (IA-RM29-1) with the measured facts — and,
/// on failure, the named failures — as its details
/// slot. The templates are the only words; everything
/// else in the line is detector data.
pub fn qa_line(core: &EditorCore, report: &QaReport) -> String {
    if report.passed {
        i18n_wire::tr(core, "export_qa_pass").replace("{details}", &report.details)
    } else {
        let details = format!("{} — {}", report.failures.join("; "), report.details);
        i18n_wire::tr(core, "export_qa_fail").replace("{details}", &details)
    }
}

thread_local! {
    /// The latest QA reports, newest last, keyed by
    /// destination (a re-export of the same file
    /// replaces its earlier report). Session state in
    /// the panel's thread-local idiom (the overwrite
    /// guard's precedent): no `State` field, so no
    /// census moves.
    static QA_REPORTS: RefCell<Vec<QaReport>> = RefCell::new(Vec::new());
    /// The residue of the last failed sequence job:
    /// how many frame files it left, and where.
    static RESIDUE: RefCell<Option<(usize, String)>> = RefCell::new(None);
    /// Expected frame counts for sequence rows, by row
    /// name, recorded when a sequence job starts (see
    /// `note_expected_sequence`).
    static EXPECTED_SEQUENCES: RefCell<Vec<(String, usize)>> = RefCell::new(Vec::new());
}

/// The stored QA reports, oldest first.
pub fn qa_reports() -> Vec<QaReport> {
    QA_REPORTS.with(|slot| slot.borrow().clone())
}

/// How many landed verdicts the session probe
/// history keeps; the oldest fall off the end. The
/// per-destination reports (`QA_REPORTS`) are the
/// current state; the history is the running record,
/// so it is allowed to be longer — but not unbounded.
pub const QA_HISTORY_MAX: usize = 20;

/// One landed probe verdict in the session history:
/// the report plus the moment its drain stored it —
/// the moment the verdict became known, which is the
/// only timestamp the probe can honestly carry (the
/// file's own mtime is Recent exports' business).
#[derive(Clone, Debug, PartialEq)]
pub struct QaHistoryEntry {
    pub report: QaReport,
    pub at: SystemTime,
}

/// A probe running on its worker (W2-44): the
/// destination in the display shape `probe_export`
/// reports it under (kept so a dead worker's failure
/// report names the same destination), and the
/// channel the worker sends its one report back on.
/// The worker owns the destination path and the
/// expectations — both plain data, read off the
/// project on the UI thread before the spawn, so the
/// worker never touches shell state.
struct QaJob {
    dest: String,
    outcome: mpsc::Receiver<QaReport>,
}

thread_local! {
    /// Probes in flight, in queue order. Session
    /// state in the panel's thread-local idiom, like
    /// the reports they land in: no `State` field, so
    /// no census moves.
    static QA_JOBS: RefCell<Vec<QaJob>> = RefCell::new(Vec::new());
    /// The session probe history, oldest first,
    /// capped at `QA_HISTORY_MAX` (see `store_report`).
    static QA_HISTORY: RefCell<Vec<QaHistoryEntry>> = RefCell::new(Vec::new());
}

/// The session probe history, oldest first.
pub fn qa_history() -> Vec<QaHistoryEntry> {
    QA_HISTORY.with(|slot| slot.borrow().clone())
}

/// One history line: the landing time (the panel's
/// `YYYY-MM-DD HH:MM` UTC shape, `home_panel`'s
/// formatter) plus the verdict line itself — so a
/// failure's named checks ride along verbatim, and
/// the only new words are the verdict templates'
/// own, exactly as on the `xc-qa` lines.
pub fn qa_history_line(core: &EditorCore, entry: &QaHistoryEntry) -> String {
    format!(
        "{} · {}",
        crate::rune_sec::home_panel::format_modified(Some(entry.at)),
        qa_line(core, &entry.report)
    )
}

/// Store one landed report: it replaces any earlier
/// report for the same destination in `QA_REPORTS`
/// and is appended to the session history, stamped
/// with the landing time; the history's oldest
/// entries fall off past `QA_HISTORY_MAX`.
fn store_report(report: QaReport) {
    QA_REPORTS.with(|slot| {
        let mut reports = slot.borrow_mut();
        reports.retain(|r| r.dest != report.dest);
        reports.push(report.clone());
    });
    QA_HISTORY.with(|slot| {
        let mut history = slot.borrow_mut();
        history.push(QaHistoryEntry {
            report,
            at: SystemTime::now(),
        });
        if history.len() > QA_HISTORY_MAX {
            let excess = history.len() - QA_HISTORY_MAX;
            history.drain(..excess);
        }
    });
}

/// Is a QA probe still running on its worker? While
/// one is, `poll_export_job` reads active, so the
/// shell keeps the `JOB_WAKE` cadence until the
/// verdict lands.
pub fn qa_probe_active() -> bool {
    QA_JOBS.with(|slot| !slot.borrow().is_empty())
}

/// Drain every landed probe report into the store
/// (see `store_report`). Returns true when at least
/// one report landed — the repaint signal. A worker
/// that dies without sending (a panic inside the
/// probe) lands a failure report naming the probe's
/// own stoppage, mirroring the export drain's
/// worker-death discipline: never a pass, never
/// silently dropped. There is deliberately no
/// cancellation: a probe is short, read-only and
/// detached (D-4's cancel stays unbuilt).
fn poll_qa_probes() -> bool {
    let mut landed: Vec<QaReport> = Vec::new();
    QA_JOBS.with(|slot| {
        let mut jobs = slot.borrow_mut();
        let mut i = 0;
        while i < jobs.len() {
            match jobs[i].outcome.try_recv() {
                Ok(report) => {
                    jobs.remove(i);
                    landed.push(report);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let job = jobs.remove(i);
                    landed.push(QaReport {
                        dest: job.dest,
                        passed: false,
                        failures: vec![
                            "the QA probe stopped before reporting a result".to_string()
                        ],
                        details: "unprobed".to_string(),
                    });
                }
                Err(mpsc::TryRecvError::Empty) => i += 1,
            }
        }
    });
    let any = !landed.is_empty();
    for report in landed {
        store_report(report);
    }
    any
}

/// Queue a probe of a just-finished export on a
/// worker thread (W2-44; RM29 ran it inline on the
/// finish path). The expectations are read off the
/// project HERE, on the caller's thread at finish
/// time — the same project state RM29 read — and the
/// worker runs `probe_export` over plain data and
/// sends the report back; `poll_qa_probes` stores it.
/// Queues only when the destination file is actually
/// there: a job whose worker claimed success for a
/// file that does not exist is the worker-death case,
/// which the finish path already reports as a
/// failure — the probe's absence there is not a pass,
/// and no probe is queued.
fn queue_probe(
    project: &stykanimate_rs::model::Project,
    dest: &Path,
    window: Option<(usize, usize)>,
    audio_possible: bool,
) {
    if !dest.is_file() {
        return;
    }
    let expect = qa_expectations(project, window, audio_possible);
    let (tx, rx) = mpsc::channel::<QaReport>();
    let path = dest.to_path_buf();
    QA_JOBS.with(|slot| {
        slot.borrow_mut().push(QaJob {
            dest: format!("./{}", display_name(dest)),
            outcome: rx,
        })
    });
    std::thread::spawn(move || {
        let report = probe_export(&path, &expect);
        let _ = tx.send(report);
    });
}

/// The stored sequence residue: `(frames left, dest)`.
pub fn residue_note() -> Option<(usize, String)> {
    RESIDUE.with(|slot| slot.borrow().clone())
}

fn store_residue(count: usize, dest: &Path) {
    let shown = format!("./{}", display_name(dest));
    RESIDUE.with(|slot| *slot.borrow_mut() = Some((count, shown)));
}

fn clear_residue() {
    RESIDUE.with(|slot| *slot.borrow_mut() = None);
}

/// How many sequence frame files a directory holds —
/// the residue a failed sequence export left behind.
pub fn sequence_residue(dest: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dest) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
        .filter(|e| is_sequence_frame(&e.path()))
        .count()
}

/// The sampled frame count a sequence job over
/// `window` will write: the full timeline's sampled
/// total, or the window's duration-weighted beat span
/// (the same span `qa_expectations` derives).
pub fn expected_sequence_frames(
    project: &stykanimate_rs::model::Project,
    window: Option<(usize, usize)>,
) -> usize {
    match window {
        Some((a, b)) => (project.beats_at_frame_time((b + 1) as f32)
            - project.beats_at_frame_time(a as f32))
        .round()
        .max(0.0) as usize,
        None => stykanimate_rs::export::sampled_frame_total(project),
    }
}

/// Record the frame count the next sequence export to
/// `row_name`'s directory is expected to write, so
/// Recent exports can mark the row incomplete when the
/// directory holds fewer (`mark_incomplete`). Called
/// when the job starts: a job that dies mid-write is
/// exactly the case the mark exists for.
pub fn note_expected_sequence(row_name: &str, expected: usize) {
    EXPECTED_SEQUENCES.with(|slot| {
        let mut rows = slot.borrow_mut();
        rows.retain(|(name, _)| name != row_name);
        rows.push((row_name.to_string(), expected));
    });
}

/// Mark rows incomplete from the registry: a sequence
/// row whose directory holds fewer files than the last
/// sequence job for it was expected to write. Rows the
/// registry does not name keep their state — the panel
/// only claims incompleteness it has a count for.
pub fn mark_incomplete(rows: &mut [ExportRow]) {
    EXPECTED_SEQUENCES.with(|slot| {
        let registry = slot.borrow();
        for row in rows.iter_mut() {
            if let Some((_, expected)) = registry.iter().find(|(name, _)| *name == row.name) {
                row.incomplete = row.count < *expected;
            }
        }
    });
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
    // RM23: the GIF writes the named destination in
    // both modes, under the overwrite guard. The full
    // mode calls the lib entry directly — it is the
    // very function `EditorCore::export_gif` calls
    // (its `render_gif` alias) — in the op's exact
    // status wording, so naming + guard hold in "All
    // frames" too without touching `editor.rs`.
    let range = range_target(&s.core, s.export_use_range);
    let dir = export_dir();
    let path = dir.join(
        file_name_for(ExportOp::Gif, &project_stem(&s.core.project))
            .expect("GIF is a file format"),
    );
    if guard_blocks(s, std::slice::from_ref(&path)) {
        return;
    }
    match range {
        Some((a, b)) => {
            let _ = std::fs::create_dir_all(&dir);
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
        None => match stykanimate_rs::export::export_gif(&s.core.project, &path) {
            Ok(n) => {
                s.core.status = format!("Exported {n}-frame GIF to {}", path.display())
            }
            Err(e) => s.core.status = format!("GIF export failed: {e}"),
        },
    }
    note_single(s, ExportOp::Gif, range);
}

/// Spritesheet with the range toggle (Phase J): the range
/// variant writes the one sheet to the named
/// destination (`./export/<project>-spritesheet.png`) and reports in
/// the contact-sheet action's voice — frame count, the
/// range, the path.
fn export_sheet_action(s: &mut State) {
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    // RM23: named destination + overwrite guard, both
    // modes — the full mode calls the lib entry
    // directly (the `EditorCore::export_sheet` op's
    // own `render_sheet` alias target) in the op's
    // exact status wording, as `export_gif_action`.
    let range = range_target(&s.core, s.export_use_range);
    let dir = export_dir();
    let path = dir.join(
        file_name_for(ExportOp::Spritesheet, &project_stem(&s.core.project))
            .expect("spritesheet is a file format"),
    );
    if guard_blocks(s, std::slice::from_ref(&path)) {
        return;
    }
    match range {
        Some((a, b)) => {
            let _ = std::fs::create_dir_all(&dir);
            match stykanimate_rs::export::export_sheet_range_pack(&s.core.project, a, b, &path) {
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
        None => match stykanimate_rs::export::export_spritesheet_pack(&s.core.project, &path) {
            Ok(n) => {
                s.core.status = format!("Exported {n}-frame spritesheet.")
            }
            Err(e) => s.core.status = format!("Spritesheet failed: {e}"),
        },
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
    let path = dir.join(
        file_name_for(ExportOp::ContactSheet, &project_stem(&core.project))
            .expect("contact sheet is a file format"),
    );
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
    // RM23: named destination + overwrite guard, both
    // modes (the full helper above builds the same
    // named path).
    let range = range_target(&s.core, s.export_use_range);
    let dir = export_dir();
    let path = dir.join(
        file_name_for(ExportOp::ContactSheet, &project_stem(&s.core.project))
            .expect("contact sheet is a file format"),
    );
    if guard_blocks(s, std::slice::from_ref(&path)) {
        return;
    }
    match range {
        Some((a, b)) => {
            let _ = std::fs::create_dir_all(&dir);
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
    // RM23: named destination + overwrite guard, both
    // modes — the full mode calls the lib entry
    // directly (the very function
    // `EditorCore::export_apng_file` calls) in the
    // op's exact status wording: its success sentence
    // names the `out/` path, so the named stem rides
    // inside that same shape.
    let range = range_target(&s.core, s.export_use_range);
    let dir = PathBuf::from("out");
    let name = file_name_for(ExportOp::Apng, &project_stem(&s.core.project))
        .expect("APNG is a file format");
    let path = dir.join(&name);
    if guard_blocks(s, std::slice::from_ref(&path)) {
        return;
    }
    match range {
        Some((a, b)) => {
            let _ = std::fs::create_dir_all(&dir);
            match stykanimate_rs::export::export_apng_range(&s.core.project, a, b, &path) {
                Ok(n) => {
                    s.core.status = format!(
                        "Exported {n}-frame APNG (frames {}–{}) to out/{name}.",
                        a + 1,
                        b + 1
                    )
                }
                Err(e) => s.core.status = format!("APNG export failed: {e}"),
            }
        }
        None => {
            let _ = std::fs::create_dir_all(&dir);
            match stykanimate_rs::export::export_apng(&s.core.project, &path) {
                Ok(_) => {
                    s.core.status = format!("Exported out/{name}.")
                }
                Err(e) => s.core.status = format!("APNG export failed: {e}"),
            }
        }
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

/// The Export delivery press (RM23): run the selected
/// preset. The MP4 presets start the delivery job
/// (the scaled entry at the preset's scale, to the
/// preset's named destination; the finish is the MP4
/// job finish, wording included). The GIF preset has
/// no progress contract — the GIF entries carry none
/// — so it runs synchronously through the scaled GIF
/// entry, under the overwrite guard, in the GIF
/// action's exact status wording. Delivery exports
/// are always full exports: a preset names a finished
/// episode file, so the loop-range toggle does not
/// apply to it (the delivery line says what it makes).
fn export_delivery_action(s: &mut State) {
    // One export at a time (Wave W, W1), as
    // `export_svg_action`.
    if export_job_active(s) {
        return;
    }
    let preset = delivery_preset();
    if preset.is_gif() {
        let dest = delivery_dest(&s.core, preset);
        if guard_blocks(s, std::slice::from_ref(&dest)) {
            return;
        }
        let _ = std::fs::create_dir_all(export_dir());
        match stykanimate_rs::export::export_gif_scaled(
            &s.core.project,
            &dest,
            delivery_scale(&s.core.project, preset),
        ) {
            Ok(n) => {
                s.core.status = format!("Exported {n}-frame GIF to {}", dest.display())
            }
            Err(e) => s.core.status = format!("GIF export failed: {e}"),
        }
        // The recap names the file the preset actually
        // wrote (its suffix differs from the plain GIF
        // row's destination).
        s.export_recap = if export_failed(&s.core) {
            failure_recap(&s.core.status, ExportOp::Gif.name())
        } else {
            single_recap(
                ExportOp::Gif.name(),
                &format!("./{}", display_name(&dest)),
            )
        };
        return;
    }
    start_delivery_job(s, preset);
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
    /// The row stands for a sequence whose directory
    /// holds fewer frames than the last sequence job
    /// for it was expected to write (RM29 — see
    /// `mark_incomplete`). Set after the scan builds
    /// the rows, never by the scan itself: the scan
    /// knows what is on disk, not what should be.
    pub incomplete: bool,
}

/// One raw scan result — (path, size in bytes, mtime,
/// file count) — the plain-data input the row model is
/// built from, so the model builders stay pure and
/// testable. Plain files carry a count of 1; the scan's
/// sequence-directory entries carry the directory's file
/// count and total size (see `scan_sequence_dir`).
pub type FileTriple = (PathBuf, u64, SystemTime, usize);


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
                incomplete: false,
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
                incomplete: false,
            });
        } else {
            rows.push(ExportRow {
                name: display_name(&single),
                size: total,
                mtime: newest,
                count: 1,
                incomplete: false,
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
    let line = if row.count > 1 {
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
    };
    // An incomplete sequence (RM29) says so on its own
    // row: the marker word rides IA-RM29-1
    // (`export_history_incomplete`) and is appended to
    // the row's template line, so a complete row's line
    // stays byte-identical to what it has always been.
    if row.incomplete {
        format!("{} · {}", line, i18n_wire::tr(core, "export_history_incomplete"))
    } else {
        line
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
    let mut rows = if keys.iter().all(|k| k.is_none()) {
        rows_from_triples(&scan_triples(&roots))
    } else {
        HISTORY_CACHE.with(|slot| {
            let mut slot = slot.borrow_mut();
            cached_history(&mut slot, keys, || rows_from_triples(&scan_triples(&roots)))
        })
    };
    // Incompleteness (RM29) is marked on the way out,
    // not in the cache: the registry of expected counts
    // can change between two builds of the same cached
    // scan (a sequence job starts, or a partial export
    // lands), and the mark must follow the registry,
    // not the scan's moment.
    mark_incomplete(&mut rows);
    rows
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
    guard: Option<PathBuf>,
) -> El {
    let handler = move |s: &mut State| {
        // One export at a time (Wave W, W1): while a
        // background job runs, a press here is a no-op.
        if export_job_active(s) {
            return;
        }
        // The overwrite guard (RM23) for the rows whose
        // destination this panel knows exactly (the
        // .piv files and .stknds): an existing master
        // stops the first press. `.stk`/`.nodes` pass
        // `None` — their file names are the selected
        // figure's, built inside the editor ops.
        if let Some(target) = &guard {
            if guard_blocks(s, std::slice::from_ref(target)) {
                return;
            }
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
            &note_with_dest(core, "export_note_sheet", ExportOp::Spritesheet),
            t,
            export_sheet_action,
        ))
        .child(range_row(
            "xc-contact",
            "table-cells-large",
            &i18n_wire::tr(core, "export_contact_sheet"),
            &note_with_dest(core, "export_note_contact", ExportOp::ContactSheet),
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
            &note_with_dest(core, "export_note_gif", ExportOp::Gif),
            t,
            export_gif_action,
        ))
        .child(range_row(
            "xc-apng",
            "film",
            &i18n_wire::tr(core, "export_apng"),
            &note_with_dest(core, "export_note_apng", ExportOp::Apng),
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
            &note_with_dest(core, "export_note_mp4", ExportOp::Mp4),
            t,
            export_mp4_action,
        ))
        .child(range_row(
            "xc-webm",
            "video",
            &i18n_wire::tr(core, "export_webm"),
            &note_with_dest(core, "export_note_webm", ExportOp::Webm),
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
            Some(export_dir().join("animation-v2.piv")),
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
            Some(export_dir().join("animation.piv")),
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
            Some(export_dir().join("animation-v4.piv")),
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
            Some(PathBuf::from("out").join("animation-v5.piv")),
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
            None,
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
            None,
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
            Some(export_dir().join("animation.stknds")),
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
        ))
        // The publish package (RM23): a third bundle
        // row in the same shape — its press starts the
        // package job (`start_publish_job`), and its
        // note names the four members it produces.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon_btn(
                    "xc-bundle-publish",
                    "video",
                    &i18n_wire::tr(core, "export_bundle_publish"),
                    false,
                    t,
                    |s: &mut State| {
                        start_publish_job(s);
                    },
                ))
                .child(
                    text(i18n_wire::tr(core, "export_note_bundle_publish"))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .wrap()
                        .grow(1.0),
                ),
        );
    panel = panel.child(bundles);

    // The Delivery section (RM23): the preset chips
    // (the range toggle's button idiom — the active
    // chip is the selected preset; a chip press is
    // inert while a job runs, so a running delivery
    // or publish job keeps the selection it started
    // under), the Export delivery press, and the
    // delivery line naming what the press will make.
    // When the delivery destination already exists,
    // the overwrite note says so BEFORE the press —
    // the guard itself still stops the first press.
    let preset = delivery_preset();
    let mut preset_row = row().key("xc-preset-row").gap(8.0).center_y();
    for p in [
        DeliveryPreset::Master,
        DeliveryPreset::Youtube1080,
        DeliveryPreset::Youtube4k,
        DeliveryPreset::Gif,
        DeliveryPreset::Shorts,
    ] {
        preset_row = preset_row.child(btn(
            p.key(),
            &i18n_wire::tr(core, p.label_key()),
            preset == p,
            t,
            move |s: &mut State| {
                if !export_job_active(s) {
                    set_delivery_preset(p);
                }
            },
        ));
    }
    let mut delivery = col()
        .gap(6.0)
        .child(design::section_label(&i18n_wire::tr(core, "export_sec_delivery"), t))
        .child(preset_row)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon_btn(
                    "xc-delivery",
                    "video",
                    &i18n_wire::tr(core, "export_delivery"),
                    false,
                    t,
                    export_delivery_action,
                ))
                .child(
                    text(delivery_text(core, preset))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .wrap()
                        .grow(1.0),
                ),
        );
    if delivery_dest(core, preset).is_file() {
        delivery = delivery.child(
            text(
                i18n_wire::tr(core, "export_overwrite_note").replace(
                    "{dest}",
                    &format!("./{}", display_name(&delivery_dest(core, preset))),
                ),
            )
            .key("xc-overwrite-note")
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
        );
    }
    if matches!(preset, DeliveryPreset::Shorts) {
        // The reframing note (W2-43, IA-W2-43-1 (i)):
        // shown only while Shorts is selected, so the
        // user reads what the centred crop keeps before
        // pressing Export delivery.
        delivery = delivery.child(
            text(i18n_wire::tr(core, "export_note_shorts"))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    if !preset.is_gif() {
        // The chapter line (W2-18): MP4 presets embed the
        // project's chapter markers; the GIF preset makes
        // no MP4, so the line belongs to the MP4 presets
        // only. Absent entirely when nothing will embed.
        if let Some(line) = delivery_chapters_text(core) {
            delivery = delivery.child(
                text(line)
                    .key("xc-delivery-chapters")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );
        }
    }
    panel = panel.child(delivery);

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

    // The in-flight QA probe (W2-64): while a probe
    // worker is still running, one indeterminate line
    // says so — the probe exposes no phase contract
    // (W2-44), so the line claims a state, never a
    // number. Before this seat the panel showed the
    // previous verdicts (or nothing) with no sign a
    // check was still running.
    if qa_probe_active() {
        panel = panel.child(
            text(i18n_wire::tr(core, "export_qa_checking"))
                .key("xc-qa-checking")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }

    // The post-export QA records (RM29): one line per
    // probed export, under the recap they sit beside —
    // the probe's verdict, never a rewrite of the recap.
    // A passing line reads secondary; a failing one
    // takes the error colour, because a success recap
    // over a broken file is exactly the P0-1 shape this
    // line exists to contradict out loud.
    let reports = qa_reports();
    if !reports.is_empty() {
        let mut qa = col().key("xc-qa").gap(2.0);
        for report in &reports {
            qa = qa.child(
                text(qa_line(core, report))
                    .font(type_::LABEL)
                    .color(if report.passed {
                        t.text_secondary
                    } else {
                        Severity::Error.color(t)
                    })
                    .wrap()
                    .w_full(),
            );
        }
        panel = panel.child(qa);
    }

    // The session probe history (W2-44): every landed
    // verdict with the time it landed, newest first.
    // The `xc-qa` lines above hold only the latest
    // verdict per destination; this is the running
    // record — including verdicts a re-export has
    // since replaced up there — so a failure that was
    // later fixed still shows it happened, and when.
    let history = qa_history();
    if !history.is_empty() {
        let mut hist = col().key("xc-qa-history").gap(2.0);
        for entry in history.iter().rev() {
            hist = hist.child(
                text(qa_history_line(core, entry))
                    .font(type_::LABEL)
                    .color(if entry.report.passed {
                        t.text_secondary
                    } else {
                        Severity::Error.color(t)
                    })
                    .wrap()
                    .w_full(),
            );
        }
        panel = panel.child(hist);
    }

    // The failed sequence's residue (RM29): how many
    // frames the attempt left behind, and where — the
    // failure's own sentence lives on the status and
    // the recap; this line only states what is on disk.
    if let Some((count, dest)) = residue_note() {
        panel = panel.child(
            text(
                i18n_wire::tr(core, "export_residue_line")
                    .replace("{count}", &count.to_string())
                    .replace("{dest}", &dest),
            )
            .key("xc-residue")
            .font(type_::LABEL)
            .color(Severity::Error.color(t))
            .wrap()
            .w_full(),
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
        // The cancel affordance (W2-64): one row under
        // the lines, present for every job shape while
        // the job runs — disabled-honest today (see
        // `cancel_row`), the StopSignal-ready chrome
        // for the D-4 build.
        progress = progress.child(cancel_row(core, job, t));
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
            ("export_note_sheet", "Frames on one sheet — {dest}"),
            ("export_note_contact", "Frames as one tile sheet — {dest}"),
            ("export_note_gif", "Animated GIF — {dest}"),
            ("export_note_apng", "Animated PNG — {dest} (out/, not export/)"),
            ("export_note_mp4", "MP4 video via ffmpeg — {dest} (full export includes audio; range export includes audio from clips inside the range — WAV or any other ffmpeg-readable format; a clip file that cannot be probed or decoded fails the range export)"),
            ("export_note_webm", "WebM video (VP9) — {dest} (full export is video-only; range export includes audio from clips inside the range)"),
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
            ("export_piv_note_segments", "Media note: .piv v2/v3/v4 export image and text segments as plain segments — only v5 keeps segment media. Not even it keeps a media's whole local payload: only the flip crosses over — the offset, scale, angle and opacity stay behind."),
            ("export_piv_note_instances", "Media note: no .piv version writes sprite or text instances — they are left out of .piv exports."),
            ("export_piv_note_both", "Media note: .piv v2/v3/v4 export image and text segments as plain segments (only v5 keeps them), and no .piv version writes sprite or text instances — they are left out of .piv exports. Neither keeps a media's whole local payload: only the flip crosses over — the offset, scale, angle and opacity stay behind."),
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
            incomplete: false,
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
            bundle_recap(BundleKind::Images, &[], "animation"),
            "Exported PNG → ./export/, JPEG → out/jpeg/, SVG → ./export/svg/, sheet → ./export/spritesheet.png"
        );
        assert_eq!(
            bundle_recap(BundleKind::Video, &[], "animation"),
            "Exported MP4 → ./export/animation.mp4, WebM → ./export/animation.webm"
        );
    }

    #[test]
    fn bundle_recap_names_failures_instead_of_claiming_them() {
        assert_eq!(
            bundle_recap(BundleKind::Video, &[ExportOp::Mp4], "animation"),
            "Exported WebM → ./export/animation.webm; failed: MP4"
        );
        assert_eq!(
            bundle_recap(BundleKind::Video, &[ExportOp::Mp4, ExportOp::Webm], "animation"),
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
            incomplete: false,
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
                "Media note: .piv v2/v3/v4 export image and text segments as plain segments — only v5 keeps segment media. Not even it keeps a media's whole local payload: only the flip crosses over — the offset, scale, angle and opacity stay behind."
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
                "Media note: .piv v2/v3/v4 export image and text segments as plain segments (only v5 keeps them), and no .piv version writes sprite or text instances — they are left out of .piv exports. Neither keeps a media's whole local payload: only the flip crosses over — the offset, scale, angle and opacity stay behind."
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
        // Roadmap Wave 2 (W2-50): a third line rides
        // under the two phase lines — the phase ETA
        // for the active phase (the encode, the later
        // phase reporting), estimated from that
        // phase's own slot only. The separation this
        // pin exists for is intact: the ETA merges no
        // counts and reads no other slot. Its text
        // expectation is built from the catalogue the
        // code reads, so it holds before and after
        // IA-W2-50-1's splice lands the keys.
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].0, "xc-progress-render");
        assert_eq!(lines[0].1, render_progress_line(&s.core, render));
        assert_eq!(lines[1].0, "xc-progress-encode");
        assert_eq!(lines[1].1, encode_progress_line(&s.core, encode));
        assert_eq!(lines[2].0, "xc-progress-eta");
        assert_eq!(
            lines[2].1,
            crate::rune_sec::i18n_wire::tr(&s.core, "export_eta_estimating").replace(
                "{phase}",
                &crate::rune_sec::i18n_wire::tr(&s.core, "export_phase_encode"),
            )
        );
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
        assert_eq!(dest, export_dir().join("untitled.mp4"));
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
        let recap = bundle_recap(BundleKind::Images, &[ExportOp::Svg], "untitled");
        assert_eq!(
            recap,
            "Exported PNG → ./export/, JPEG → out/jpeg/, sheet → ./export/untitled-spritesheet.png; failed: SVG"
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
        let recap = bundle_recap(BundleKind::Video, &[], "untitled");
        assert_eq!(recap, "Exported MP4 → ./export/untitled.mp4, WebM → ./export/untitled.webm");
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
        let recap = bundle_recap(BundleKind::Images, &plan_bundle(BundleKind::Images), "untitled");
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
        // The readout is the bundle line, built from
        // the bundle snapshot — no phase line, no
        // merged figure — plus (Roadmap Wave 2,
        // W2-50) the bundle slot's own ETA line under
        // it, estimated from the bundle slot's samples
        // alone: the stray render store surfaces
        // nowhere, not even in the estimate. The text
        // expectation is built from the catalogue the
        // code reads, so it holds before and after
        // IA-W2-50-1's splice.
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, "xc-progress-bundle");
        assert_eq!(lines[0].1, bundle_progress_line(&s.core, bundle));
        assert_eq!(lines[1].0, "xc-progress-eta");
        assert_eq!(
            lines[1].1,
            crate::rune_sec::i18n_wire::tr(&s.core, "export_eta_estimating")
                .replace("{phase}", "images bundle")
        );
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
    fn job_lines_keys_are_exactly_the_tuple_carried_population() {
        // Wave AC (AC9): the F6 artefact, pinned as a
        // closed set. `job_lines` returns its keys as
        // (key, line) tuple data, applied through a
        // variable at the readout's render site,
        // so no gate extractor pattern — a key literal
        // in a `.key()` call, a helper-call literal,
        // a `key:` field — captures them: the line
        // keys ride OUTSIDE the phase_j_checks counter
        // by design, disclosed in that script's floor
        // ledger at every ratchet since Wave W. The
        // per-shape pins above assert each key's value
        // at its own site; this pin asserts the
        // population: across every job shape, the union
        // of returned keys is exactly the four known
        // keys, so a fifth tuple-carried key cannot
        // join the artefact silently — it fails here
        // until it is named in the ledger and in this
        // set deliberately.
        //
        // Roadmap Wave 2 (W2-50): the fifth key IS
        // named, deliberately — `xc-progress-eta`,
        // the phase-ETA line `job_lines` appends
        // while a phase is actively reporting (the
        // MP4 and bundle scenarios below produce it
        // in its estimating form). It rides outside
        // the gate counter by the same F6 mechanism
        // as the other four; IA-W2-50-2 carries the
        // ledger disclosure for the gate script.
        let core = EditorCore::new();
        let mk = |kind: JobKind| {
            let (_tx, rx) = mpsc::channel::<ExportJobOutcome>();
            ExportJob {
                kind,
                dest: PathBuf::from("out-test"),
                render: Arc::new(PhaseProgress::default()),
                encode: Arc::new(PhaseProgress::default()),
                bundle: Arc::new(PhaseProgress::default()),
                outcome: rx,
                shown_render: PhaseSnapshot::default(),
                shown_encode: PhaseSnapshot::default(),
                shown_bundle: PhaseSnapshot::default(),
            }
        };
        let mut seen: Vec<&'static str> = Vec::new();
        let mut collect = |job: &ExportJob| {
            for (key, _line) in job_lines(&core, job) {
                if !seen.contains(&key) {
                    seen.push(key);
                }
            }
        };
        // A single-format job before any event: the
        // working line only.
        collect(&mk(JobKind::PngFull));
        // A single-format job with both phases
        // reported: the render and encode lines.
        let job = mk(JobKind::Mp4Full);
        job.render
            .store_render(stykanimate_rs::export::RenderProgressEvent {
                rendered_frames: 1,
                total_frames: 2,
                finished: false,
            });
        job.encode
            .store_encode(stykanimate_rs::export::EncodeProgressEvent {
                encoded_frames: 1,
                total_frames: 2,
                finished: false,
            });
        collect(&job);
        // A bundle job before any event: the working
        // line again — no new key.
        collect(&mk(JobKind::BundleImages));
        // A bundle job with a bundle event reported:
        // the bundle line.
        let job = mk(JobKind::BundleVideo);
        job.bundle
            .store_bundle(stykanimate_rs::export::BundleProgressEvent {
                completed_frames: 1,
                total_frames: 2,
                finished: false,
            });
        collect(&job);
        seen.sort_unstable();
        assert_eq!(
            seen,
            vec![
                "xc-progress-bundle",
                "xc-progress-encode",
                "xc-progress-eta",
                "xc-progress-render",
                "xc-progress-working",
            ]
        );
    }

    // ---- Phase ETAs (Roadmap Wave 2, W2-50) ----
    // The estimator is a pure function of synthetic
    // samples; the annex pins drive the `store_*_at`
    // seam with injected arrival times. No pin sleeps
    // and none reads a wall clock for its evidence.

    /// A job built directly over fresh slots (the AC9
    /// pin's construction), for ETA readout scenarios.
    fn eta_job(kind: JobKind) -> ExportJob {
        let (_tx, rx) = mpsc::channel::<ExportJobOutcome>();
        ExportJob {
            kind,
            dest: PathBuf::from("out-test"),
            render: Arc::new(PhaseProgress::default()),
            encode: Arc::new(PhaseProgress::default()),
            bundle: Arc::new(PhaseProgress::default()),
            outcome: rx,
            shown_render: PhaseSnapshot::default(),
            shown_encode: PhaseSnapshot::default(),
            shown_bundle: PhaseSnapshot::default(),
        }
    }

    fn render_ev(
        done: usize,
        total: usize,
        finished: bool,
    ) -> stykanimate_rs::export::RenderProgressEvent {
        stykanimate_rs::export::RenderProgressEvent {
            rendered_frames: done,
            total_frames: total,
            finished,
        }
    }

    fn encode_ev(
        done: usize,
        total: usize,
        finished: bool,
    ) -> stykanimate_rs::export::EncodeProgressEvent {
        stykanimate_rs::export::EncodeProgressEvent {
            encoded_frames: done,
            total_frames: total,
            finished,
        }
    }

    fn bundle_ev(
        done: usize,
        total: usize,
        finished: bool,
    ) -> stykanimate_rs::export::BundleProgressEvent {
        stykanimate_rs::export::BundleProgressEvent {
            completed_frames: done,
            total_frames: total,
            finished,
        }
    }

    #[test]
    fn eta_estimator_stays_estimating_without_a_measured_rate() {
        // No samples at all.
        assert_eq!(estimate_eta(&[], 100), EtaEstimate::Estimating);
        // One sample: a point, not a rate.
        assert_eq!(estimate_eta(&[(0.0, 1)], 100), EtaEstimate::Estimating);
        // Plenty of advance over too short a span: a
        // fast phase's rate is not quotable from 1.5 s
        // of evidence.
        assert_eq!(
            estimate_eta(&[(0.0, 0), (1.5, 60)], 100),
            EtaEstimate::Estimating
        );
        // A long span that barely advanced: two or
        // three frames of evidence would swing the
        // estimate by whole minutes either way.
        assert_eq!(
            estimate_eta(&[(0.0, 0), (6.0, 3)], 100),
            EtaEstimate::Estimating
        );
        // No work left: the terminal is in flight and
        // there is nothing to estimate.
        assert_eq!(
            estimate_eta(&[(0.0, 0), (6.0, 100)], 100),
            EtaEstimate::Estimating
        );
        // A zero total (an empty project's terminal
        // shape) never divides.
        assert_eq!(estimate_eta(&[(0.0, 0), (6.0, 0)], 0), EtaEstimate::Estimating);
    }

    #[test]
    fn eta_estimator_quotes_a_steady_rate_as_a_point() {
        // 5 frames/s throughout: the overall and
        // window rates agree, so one number — the
        // remaining 80 frames at 5/s.
        let est = estimate_eta(&[(0.0, 0), (2.0, 10), (4.0, 20), (6.0, 30)], 110);
        assert_eq!(
            est,
            EtaEstimate::Remaining {
                point_secs: 16.0,
                low_secs: 16.0,
                high_secs: 16.0,
                bracketed: false,
            }
        );
    }

    #[test]
    fn eta_estimator_brackets_when_the_rate_moves() {
        // Accelerating: 60 frames in 14 s overall, but
        // 40 of them in the last 4 s of the window —
        // the window says 8 s left, the whole phase
        // ~18.7 s; both ends are shown and the point
        // is the recent estimate.
        let est = estimate_eta(&[(0.0, 0), (10.0, 20), (14.0, 60)], 140);
        let (point, low, high, bracketed) = match est {
            EtaEstimate::Remaining {
                point_secs,
                low_secs,
                high_secs,
                bracketed,
            } => (point_secs, low_secs, high_secs, bracketed),
            other => panic!("expected a remaining estimate, got {other:?}"),
        };
        assert!(bracketed);
        assert_eq!(point, 8.0);
        assert_eq!(low, 8.0);
        assert!((high - 80.0 / (60.0 / 14.0)).abs() < 1e-9);
        // Decelerating: the mirror image — the recent
        // rate is the slow one, so the point is the
        // bracket's high end.
        let est = estimate_eta(&[(0.0, 0), (4.0, 40), (14.0, 60)], 140);
        let (point, low, high, bracketed) = match est {
            EtaEstimate::Remaining {
                point_secs,
                low_secs,
                high_secs,
                bracketed,
            } => (point_secs, low_secs, high_secs, bracketed),
            other => panic!("expected a remaining estimate, got {other:?}"),
        };
        assert!(bracketed);
        assert_eq!(point, 40.0);
        assert_eq!(high, 40.0);
        assert!((low - 80.0 / (60.0 / 14.0)).abs() < 1e-9);
    }

    #[test]
    fn eta_estimator_collapses_close_rates_to_the_recent_point() {
        // Overall ~4.71 frames/s (21.2 s left), recent
        // window 4 frames/s (25 s left): within the
        // bracket ratio, so one number — the recent
        // (freshest) estimate, with the ends kept for
        // the record.
        let est = estimate_eta(&[(0.0, 0), (10.0, 50), (14.0, 66)], 166);
        let (point, low, high, bracketed) = match est {
            EtaEstimate::Remaining {
                point_secs,
                low_secs,
                high_secs,
                bracketed,
            } => (point_secs, low_secs, high_secs, bracketed),
            other => panic!("expected a remaining estimate, got {other:?}"),
        };
        assert!(!bracketed);
        assert_eq!(point, 25.0);
        assert_eq!(high, 25.0);
        assert!((low - 100.0 / (66.0 / 14.0)).abs() < 1e-9);
    }

    #[test]
    fn eta_time_text_quantises_to_five_seconds_and_rounds_outward() {
        assert_eq!(eta_time_text(16.0, EtaRound::Nearest), "0:15");
        assert_eq!(eta_time_text(18.0, EtaRound::Nearest), "0:20");
        assert_eq!(eta_time_text(61.0, EtaRound::Down), "1:00");
        assert_eq!(eta_time_text(61.0, EtaRound::Up), "1:05");
        assert_eq!(eta_time_text(125.0, EtaRound::Nearest), "2:05");
        // A positive estimate never reads 0:00.
        assert_eq!(eta_time_text(2.0, EtaRound::Down), "0:05");
        assert_eq!(eta_time_text(0.0, EtaRound::Nearest), "0:00");
    }

    #[test]
    fn eta_samples_are_thinned_anchored_and_keep_the_terminal() {
        let slot = PhaseProgress::default();
        let t0 = Instant::now();
        let at = |ms: u64| t0 + std::time::Duration::from_millis(ms);
        slot.store_render_at(render_ev(1, 10, false), at(0));
        // 100 ms later: inside the thinning interval,
        // so no sample — the count itself still lands
        // in the snapshot; only the annex thins.
        slot.store_render_at(render_ev(2, 10, false), at(100));
        assert_eq!(slot.snapshot().done, 2);
        slot.store_render_at(render_ev(3, 10, false), at(300));
        // The terminal records even inside the
        // interval, so the measured span ends at the
        // phase's true end.
        slot.store_render_at(render_ev(10, 10, true), at(350));
        let samples = slot.eta_samples();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], (0.0, 1));
        assert_eq!(samples[1].1, 3);
        assert!((samples[1].0 - 0.3).abs() < 1e-6);
        assert_eq!(samples[2].1, 10);
        assert!((samples[2].0 - 0.35).abs() < 1e-6);
        // A regressive count is refused: the annex
        // never learns backwards. (No contract
        // regresses; the guard keeps a broken one
        // from poisoning the rates.)
        let slot = PhaseProgress::default();
        slot.store_render_at(render_ev(5, 10, false), at(0));
        slot.store_render_at(render_ev(4, 10, false), at(1000));
        assert_eq!(slot.eta_samples().len(), 1);
    }

    #[test]
    fn job_phases_match_the_pipelines_measured_stages() {
        // The census: every job shape's measured
        // phases, in pipeline order. A new job shape
        // extends `job_phases`' match (the compiler
        // insists) and this table in the same commit;
        // a new measured pipeline phase adds a
        // contract, a slot and an `EtaPhase` — and
        // fails here until the table learns it.
        let cases: [(JobKind, &[EtaPhase]); 12] = [
            (JobKind::PngFull, &[EtaPhase::Render]),
            (JobKind::PngRange(0, 9), &[EtaPhase::Render]),
            (JobKind::JpegFull, &[EtaPhase::Render]),
            (JobKind::JpegRange(0, 9), &[EtaPhase::Render]),
            (JobKind::Mp4Full, &[EtaPhase::Render, EtaPhase::Encode]),
            (JobKind::Mp4Range(0, 9), &[EtaPhase::Render, EtaPhase::Encode]),
            (JobKind::WebmFull, &[EtaPhase::Render, EtaPhase::Encode]),
            (JobKind::WebmRange(0, 9), &[EtaPhase::Render, EtaPhase::Encode]),
            (
                JobKind::Mp4Preset(DeliveryPreset::Master),
                &[EtaPhase::Render, EtaPhase::Encode],
            ),
            (JobKind::BundleImages, &[EtaPhase::Bundle]),
            (JobKind::BundleVideo, &[EtaPhase::Bundle]),
            (JobKind::BundlePublish, &[EtaPhase::Bundle]),
        ];
        for (kind, phases) in cases {
            assert_eq!(job_phases(kind), phases, "phases for {kind:?}");
        }
        assert_eq!(
            EtaPhase::ALL,
            [EtaPhase::Render, EtaPhase::Encode, EtaPhase::Bundle]
        );
        // Each phase's slot is its own: storing the
        // phase's contract event through `phase_slot`
        // marks exactly that phase seen — and, while
        // unfinished, active.
        for phase in EtaPhase::ALL {
            let kind = match phase {
                EtaPhase::Render => JobKind::PngFull,
                EtaPhase::Encode => JobKind::Mp4Full,
                EtaPhase::Bundle => JobKind::BundleVideo,
            };
            let job = eta_job(kind);
            match phase {
                EtaPhase::Render => {
                    phase_slot(&job, phase).store_render(render_ev(1, 10, false))
                }
                EtaPhase::Encode => {
                    phase_slot(&job, phase).store_encode(encode_ev(1, 10, false))
                }
                EtaPhase::Bundle => {
                    phase_slot(&job, phase).store_bundle(bundle_ev(1, 10, false))
                }
            }
            assert!(phase_slot(&job, phase).snapshot().seen);
            for other in EtaPhase::ALL {
                if other != phase && job_phases(kind).contains(&other) {
                    assert!(
                        !phase_slot(&job, other).snapshot().seen,
                        "{other:?} must stay unseen when {phase:?} reports"
                    );
                }
            }
            assert_eq!(active_phase(&job), Some(phase));
        }
    }

    #[test]
    fn job_lines_carry_the_eta_line_with_phase_and_basis_named() {
        // The ETA line's catalogue keys ride
        // IA-W2-50-1's splice (the Q7 pattern, like the
        // progress-labels pin above): until the keys
        // land, `tr` falls back to the key text and
        // the text assertions below fail by design;
        // the key/structure assertions hold either way.
        let core = EditorCore::new();
        let t0 = Instant::now();
        let at = |s: u64| t0 + std::time::Duration::from_secs(s);

        // A video job mid-render: 20 frames in 4 s
        // since the first event → 5 frames/s → ~15 s
        // left for rendering, and the line names the
        // unestimated encoding that follows.
        let job = eta_job(JobKind::Mp4Full);
        job.render.store_render_at(render_ev(1, 100, false), at(0));
        job.render.store_render_at(render_ev(21, 100, false), at(4));
        let lines = job_lines(&core, &job);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, "xc-progress-render");
        assert_eq!(lines[1].0, "xc-progress-eta");
        assert_eq!(
            lines[1].1,
            "Rendering time left: about 0:15 (at the rate measured so far) Encoding follows and is not estimated yet."
        );

        // The same render in a sequence job: no
        // encoding follows, and the line does not
        // claim one.
        let job = eta_job(JobKind::PngFull);
        job.render.store_render_at(render_ev(1, 100, false), at(0));
        job.render.store_render_at(render_ev(21, 100, false), at(4));
        let lines = job_lines(&core, &job);
        assert_eq!(
            lines[1].1,
            "Rendering time left: about 0:15 (at the rate measured so far)"
        );

        // Encode active: the render terminal is
        // stored and the encode's own samples drive
        // its line — with no follow note, the encode
        // being the last measured phase.
        let job = eta_job(JobKind::Mp4Full);
        job.render.store_render_at(render_ev(100, 100, true), at(0));
        job.encode.store_encode_at(encode_ev(1, 100, false), at(0));
        job.encode.store_encode_at(encode_ev(21, 100, false), at(4));
        let lines = job_lines(&core, &job);
        let eta = lines
            .iter()
            .find(|(k, _)| *k == "xc-progress-eta")
            .expect("eta line present");
        assert_eq!(
            eta.1,
            "Encoding time left: about 0:15 (at the rate measured so far)"
        );

        // Before a quotable rate exists, the line says
        // the estimate is still forming, in the active
        // phase's name.
        let job = eta_job(JobKind::PngFull);
        job.render.store_render_at(render_ev(1, 100, false), at(0));
        let lines = job_lines(&core, &job);
        assert_eq!(lines[1].0, "xc-progress-eta");
        assert_eq!(lines[1].1, "Rendering time left: estimating…");

        // No events yet: no ETA line at all — the
        // working line stands alone.
        let job = eta_job(JobKind::PngFull);
        let lines = job_lines(&core, &job);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-working");

        // A bundle job estimates over its one combined
        // count, named by the bundle's slot-data name.
        let job = eta_job(JobKind::BundleVideo);
        job.bundle.store_bundle_at(bundle_ev(1, 100, false), at(0));
        job.bundle.store_bundle_at(bundle_ev(21, 100, false), at(4));
        let lines = job_lines(&core, &job);
        assert_eq!(lines[0].0, "xc-progress-bundle");
        assert_eq!(
            lines[1].1,
            "video bundle time left: about 0:15 (at the rate measured so far)"
        );
    }

    #[test]
    fn every_job_shape_reads_cancel_unavailable_until_a_signal_lands() {
        // W2-64: the cancel seam's standing answer. No
        // stop signal exists (AC8's `StopSignal` is a
        // design only; D-4 leaves its build to a user
        // ruling), so every job shape — single-format,
        // delivery preset, bundle, publish package —
        // reads `Unavailable`, the state `cancel_row`
        // renders as the inert button plus its reason
        // note. When a signal lands, this pin is
        // amended in the same commit that arms it,
        // never before.
        let mut s = crate::init();
        for (kind, result) in [
            (JobKind::PngFull, Ok(ExportJobResult::Frames(0))),
            (
                JobKind::Mp4Preset(DeliveryPreset::Master),
                Err("test outcome".to_string()),
            ),
            (
                JobKind::BundleImages,
                Ok(ExportJobResult::Bundle(Vec::new())),
            ),
            (
                JobKind::BundlePublish,
                Ok(ExportJobResult::Publish(Vec::new())),
            ),
        ] {
            let release = blocked_job(&mut s, kind, result);
            let job = s.export_job.as_ref().expect("job running");
            assert_eq!(job_cancel(job), JobCancel::Unavailable, "{kind:?}");
            // The row itself constructs under both
            // themes — construction is the assertable
            // level for element internals.
            let _ = cancel_row(&s.core, job, &rune::tok::theme(true));
            let _ = cancel_row(&s.core, job, &rune::tok::theme(false));
            release.send(()).expect("worker listening");
            poll_until_finished(&mut s);
        }
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
            bundle_recap(BundleKind::Images, &[], "untitled")
        );
    }

    // ---- Named destinations, guard, delivery, publish ----
    // (Roadmap Wave 1, RM23 — P-13)
    // The naming, scale and recap builders are pure;
    // the guard's state machine is driven through
    // `guard_blocks` with real temp files (existence
    // is its one filesystem input); the publish finish
    // is driven through the job seam like the bundles'.

    /// A project with the given name (the demo project
    /// otherwise).
    fn named_project(name: &str) -> stykanimate_rs::model::Project {
        let mut core = EditorCore::new();
        core.project.name = name.to_string();
        core.project
    }

    #[test]
    fn project_stem_sanitises_names_and_falls_back() {
        assert_eq!(project_stem(&named_project("Untitled")), "untitled");
        assert_eq!(project_stem(&named_project("The Last Promise")), "the-last-promise");
        assert_eq!(project_stem(&named_project("Ep. 12 — Finale!")), "ep-12-finale");
        assert_eq!(project_stem(&named_project("my_ep  2")), "my_ep-2");
        assert_eq!(project_stem(&named_project("  --A--  ")), "a");
        // No alphanumeric characters at all: the
        // historical stem, so the destinations are the
        // ones the panel has always written.
        assert_eq!(project_stem(&named_project("!!!")), "animation");
        assert_eq!(project_stem(&named_project("")), "animation");
    }

    #[test]
    fn named_destinations_carry_the_stem_and_match_legacy_at_animation() {
        // With the fallback stem the named destinations
        // are the legacy ones byte for byte — the
        // naming changes nothing for an unnamed-feeling
        // project, and everything for a named one.
        for op in [
            ExportOp::Png,
            ExportOp::Jpeg,
            ExportOp::Svg,
            ExportOp::Spritesheet,
            ExportOp::ContactSheet,
            ExportOp::Gif,
            ExportOp::Apng,
            ExportOp::Mp4,
            ExportOp::Webm,
        ] {
            assert_eq!(op.dest_named("animation"), op.dest(), "{op:?}");
            let named = op.dest_named("my-episode");
            match file_name_for(op, "my-episode") {
                Some(file) => assert!(named.ends_with(&file), "{op:?}: {named}"),
                None => assert_eq!(named, op.dest(), "{op:?} writes a directory"),
            }
        }
        assert_eq!(ExportOp::Mp4.dest_named("my-episode"), "./export/my-episode.mp4");
        assert_eq!(ExportOp::Apng.dest_named("my-episode"), "out/my-episode.apng");
        assert_eq!(
            ExportOp::Spritesheet.dest_named("my-episode"),
            "./export/my-episode-spritesheet.png"
        );
    }

    #[test]
    fn delivery_scale_dims_and_names_follow_the_preset() {
        // The demo stage is 960×720.
        let project = named_project("Demo");
        assert_eq!(delivery_scale(&project, DeliveryPreset::Master), 1.0);
        assert_eq!(delivery_scale(&project, DeliveryPreset::Youtube1080), 1.5);
        assert_eq!(delivery_scale(&project, DeliveryPreset::Youtube4k), 3.0);
        assert_eq!(delivery_scale(&project, DeliveryPreset::Gif), 0.5);
        assert_eq!(delivery_dims(&project, DeliveryPreset::Master), (960, 720));
        assert_eq!(delivery_dims(&project, DeliveryPreset::Youtube1080), (1440, 1080));
        assert_eq!(delivery_dims(&project, DeliveryPreset::Youtube4k), (2880, 2160));
        assert_eq!(delivery_dims(&project, DeliveryPreset::Gif), (480, 360));
        assert_eq!(delivery_file_name("demo", DeliveryPreset::Master), "demo.mp4");
        assert_eq!(
            delivery_file_name("demo", DeliveryPreset::Youtube1080),
            "demo-1080p.mp4"
        );
        assert_eq!(delivery_file_name("demo", DeliveryPreset::Youtube4k), "demo-4k.mp4");
        assert_eq!(delivery_file_name("demo", DeliveryPreset::Gif), "demo.gif");
        // Roadmap Wave 2 (W2-43, IA-W2-43-1): the Shorts
        // preset's dims are the core crop window of the
        // 1920-tall reframe over the 960x720 demo stage
        // (full frame 2560x1920, window 1080 wide).
        assert_eq!(delivery_dims(&project, DeliveryPreset::Shorts), (1080, 1920));
        assert_eq!(
            delivery_file_name("demo", DeliveryPreset::Shorts),
            "demo-shorts.mp4"
        );
        // A stage with no usable height falls back to
        // the unscaled render rather than a NaN scale.
        let mut flat = named_project("Demo");
        flat.stage = [960.0, 0.0];
        assert_eq!(delivery_scale(&flat, DeliveryPreset::Youtube1080), 1.0);
    }

    #[test]
    fn delivery_and_publish_labels_exist_in_the_catalogue() {
        // RM23 (IA-RM23-2): the Delivery section's
        // catalogue keys — the section heading, the
        // four preset labels, the export button, the
        // publish row's label + note, and the two
        // composed templates in stored form. Like the
        // other IA pins in this module, this passes
        // only once the integrator inserts the keys
        // (the Q7 pattern).
        let core = EditorCore::new();
        for (key, literal) in [
            ("export_sec_delivery", "Delivery"),
            ("export_preset_master", "Project master"),
            ("export_preset_youtube_1080", "YouTube 1080p"),
            ("export_preset_youtube_4k", "YouTube 4K"),
            ("export_preset_gif", "Community GIF"),
            ("export_delivery", "Export delivery"),
            ("export_bundle_publish", "Publish package"),
            (
                "export_note_bundle_publish",
                "MP4 + SRT/VTT captions + thumbnail still in one press. Always exports in full — the range toggle above applies only to the single-format buttons.",
            ),
            (
                "export_delivery_line",
                "Delivery: {preset} — {width}×{height} → {dest}",
            ),
            (
                "export_overwrite_note",
                "Heads up: {dest} already exists — the export will ask for a second press before overwriting it.",
            ),
            (
                "export_delivery_chapters",
                "Chapter markers: {count} will be embedded in the MP4.",
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
        // The composed delivery line through the public
        // helper: the demo project at the master preset.
        assert_eq!(
            delivery_text(&core, DeliveryPreset::Master),
            "Delivery: Project master — 960×720 → ./export/untitled.mp4"
        );
    }

    #[test]
    fn delivery_chapters_line_counts_the_embeddable_chapters() {
        // Roadmap Wave 2, W2-18: the chapterless demo
        // shows no line at all; with chapters authored,
        // the line names the count the core's own
        // resolution will embed — a chapter fully
        // shadowed by an earlier one (first match owns
        // its frames) does not inflate the count. The
        // composed sentence resolves through the
        // `export_delivery_chapters` template, so like
        // the pin above it passes once IA-W2-18-3 lands
        // (the Q7 pattern).
        let mut core = EditorCore::new();
        assert_eq!(delivery_chapters_text(&core), None);
        core.project.chapters = vec![
            stykanimate_rs::model::Chapter {
                title: "One".to_string(),
                start_frame: 0,
                end_frame: 3,
            },
            stykanimate_rs::model::Chapter {
                title: "Shadowed".to_string(),
                start_frame: 1,
                end_frame: 2,
            },
            stykanimate_rs::model::Chapter {
                title: "Two".to_string(),
                start_frame: 3,
                end_frame: 5,
            },
        ];
        assert_eq!(
            delivery_chapters_text(&core).unwrap(),
            "Chapter markers: 2 will be embedded in the MP4."
        );
    }

    /// Create a real file in the temp dir (the guard's
    /// existence fixture) and return its path; the
    /// caller removes it.
    fn existing_target(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rs-rm23-guard-{tag}-{}.mp4",
            std::process::id()
        ));
        std::fs::write(&path, b"master").expect("create the guard fixture");
        path
    }

    #[test]
    fn overwrite_guard_line_names_the_first_and_counts_the_rest() {
        let a = PathBuf::from("/x/export/ep.mp4");
        let b = PathBuf::from("/x/export/ep.webm");
        assert_eq!(
            overwrite_guard_line(&[a.clone()]),
            "Export target already exists: /x/export/ep.mp4 — press the same export again to overwrite it."
        );
        assert_eq!(
            overwrite_guard_line(&[a, b]),
            "Export target already exists: /x/export/ep.mp4 (and 1 more) — press the same export again to overwrite it."
        );
        // The detectors read the guard honestly: not a
        // success shape, not a failure shape.
        let line = overwrite_guard_line(&[PathBuf::from("/x/export/ep.mp4")]);
        assert!(!line.starts_with("Exported "), "{line}");
        assert!(!line.to_lowercase().contains("failed"), "{line}");
    }

    #[test]
    fn overwrite_guard_blocks_the_first_press_and_arms_the_second() {
        let mut s = crate::init();
        let target = existing_target("arm");
        let targets = vec![target.clone()];
        // First press: blocked, nothing exported, the
        // guard sentence is the status AND the recap.
        assert!(guard_blocks(&mut s, &targets));
        let line = overwrite_guard_line(&existing_targets(&targets));
        assert_eq!(s.core.status, line);
        assert_eq!(s.export_recap, line);
        // Second press of the same export: proceeds,
        // and the arm is spent — a third press blocks
        // again (the file still exists).
        assert!(!guard_blocks(&mut s, &targets));
        assert!(guard_blocks(&mut s, &targets));
        // A different target set replaces the arm: it
        // blocks on its own first press even right
        // after the other set was armed.
        let other = existing_target("other");
        let other_targets = vec![other.clone()];
        assert!(guard_blocks(&mut s, &other_targets));
        assert!(!guard_blocks(&mut s, &other_targets));
        // No existing targets: never blocks.
        let missing = PathBuf::from("/nonexistent-dir-rm23/nope.mp4");
        assert!(!guard_blocks(&mut s, &[missing]));
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(&other);
    }

    #[test]
    fn publish_targets_and_recap_name_every_member() {
        let core = EditorCore::new();
        let targets = publish_targets(&core, DeliveryPreset::Master);
        let names: Vec<String> = targets
            .iter()
            .map(|p| p.file_name().expect("file target").to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "untitled.mp4",
                "untitled.srt",
                "untitled.vtt",
                "untitled-thumbnail.png"
            ]
        );
        // The GIF preset cannot name the package's MP4
        // leg: it falls back to the master name.
        let gif_targets = publish_targets(&core, DeliveryPreset::Gif);
        assert_eq!(gif_targets[0], targets[0]);
        // Roadmap Wave 2 (W2-43, IA-W2-43-1): the Shorts
        // selection names the master leg too — a package
        // carries a landscape MP4 master by definition.
        let shorts_targets = publish_targets(&core, DeliveryPreset::Shorts);
        assert_eq!(shorts_targets[0], targets[0]);
        // A height preset names the MP4 member with
        // its suffix.
        let hd_targets = publish_targets(&core, DeliveryPreset::Youtube1080);
        assert!(hd_targets[0].ends_with("untitled-1080p.mp4"));
        assert_eq!(
            publish_recap(&[], "untitled", "untitled.mp4"),
            "Exported MP4 → ./export/untitled.mp4, SRT → ./export/untitled.srt, VTT → ./export/untitled.vtt, thumbnail → ./export/untitled-thumbnail.png"
        );
        assert_eq!(
            publish_recap(&["SRT"], "untitled", "untitled.mp4"),
            "Exported MP4 → ./export/untitled.mp4, VTT → ./export/untitled.vtt, thumbnail → ./export/untitled-thumbnail.png; failed: SRT"
        );
        assert_eq!(
            publish_recap(&PUBLISH_MEMBERS, "untitled", "untitled.mp4"),
            "Exported nothing; failed: MP4, SRT, VTT, thumbnail"
        );
        // The classifier reads a partial publish recap
        // as an error, exactly like a bundle's.
        assert_eq!(
            recap_severity(&publish_recap(&["SRT"], "untitled", "untitled.mp4")),
            Severity::Error
        );
        assert_eq!(
            recap_severity(&publish_recap(&[], "untitled", "untitled.mp4")),
            Severity::Success
        );
    }

    #[test]
    fn publish_job_finish_writes_the_recap_to_status_and_recap() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundlePublish,
            Ok(ExportJobResult::Publish(vec!["VTT"])),
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
        // The bundle finish discipline: the recap IS
        // the status and the stored recap — the failed
        // member named, the rest claimed with their
        // (named) destinations.
        let recap = publish_recap(&["VTT"], "untitled", "untitled.mp4");
        assert_eq!(s.core.status, recap);
        assert_eq!(s.export_recap, recap);
    }

    #[test]
    fn publish_worker_that_never_reports_fails_every_member() {
        let mut s = crate::init();
        let (tx, rx) = mpsc::channel::<ExportJobOutcome>();
        drop(tx);
        s.export_job = Some(ExportJob {
            kind: JobKind::BundlePublish,
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
        let recap = publish_recap(&PUBLISH_MEMBERS, "untitled", "untitled.mp4");
        assert_eq!(recap, "Exported nothing; failed: MP4, SRT, VTT, thumbnail");
        assert_eq!(s.core.status, recap);
        assert_eq!(s.export_recap, recap);
    }

    #[test]
    fn publish_job_reads_its_bundle_slot_like_a_bundle() {
        let mut s = crate::init();
        let release = blocked_job(
            &mut s,
            JobKind::BundlePublish,
            Ok(ExportJobResult::Publish(Vec::new())),
        );
        let job = s.export_job.as_ref().expect("job running");
        // Before any member reports: the working line
        // names the package (slot data, not a label).
        let lines = job_lines(&s.core, job);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, "xc-progress-working");
        assert_eq!(
            lines[0].1,
            crate::rune_sec::i18n_wire::tr(&s.core, "export_progress_working")
                .replace("{name}", "publish package")
        );
        release.send(()).expect("worker listening");
        poll_until_finished(&mut s);
    }

    #[test]
    fn delivery_preset_job_kind_maps_to_the_mp4_finish() {
        // The preset kind is an MP4 job in every
        // dispatch table the finish and the readout
        // consult — never a bundle, never ranged.
        let kind = JobKind::Mp4Preset(DeliveryPreset::Youtube1080);
        assert_eq!(kind.op(), ExportOp::Mp4);
        assert_eq!(kind.bundle_kind(), None);
        assert!(!kind.publish());
        assert_eq!(kind.range(), None);
        assert!(JobKind::BundlePublish.publish());
        assert_eq!(JobKind::BundlePublish.bundle_kind(), None);
    }

    // ---- Post-export QA probe (RM29, P-16) ----

    fn healthy_measurements() -> QaMeasurements {
        QaMeasurements {
            probe: Some(VideoProbe {
                duration_secs: 2.0,
                has_video: true,
                has_audio: true,
                frames: Some(48),
                width: 960,
                height: 720,
            }),
            audio: Some((0.5, 0.2)),
            lumas: vec![0.4, 0.5, 0.3, 0.45],
            ink: None,
        }
    }

    fn expect_with_audio() -> QaExpectations {
        QaExpectations {
            duration_secs: 2.0,
            frames: Some(48),
            audio_expected: true,
            burn_in: None,
        }
    }

    #[test]
    fn ffprobe_json_parses_streams_frames_and_dimensions() {
        let json = r#"{"streams":[
            {"codec_type":"video","width":960,"height":720,"nb_frames":"48","avg_frame_rate":"24/1"},
            {"codec_type":"audio","nb_frames":"84"}
        ],"format":{"duration":"2.000000"}}"#;
        let probe = parse_ffprobe_json(json).expect("valid ffprobe output parses");
        assert!(probe.has_video && probe.has_audio);
        assert_eq!(probe.frames, Some(48));
        assert_eq!((probe.width, probe.height), (960, 720));
        assert_eq!(probe.duration_secs, 2.0);
    }

    #[test]
    fn ffprobe_json_derives_frames_when_the_container_omits_them() {
        // WebM carries no nb_frames: the count derives
        // from the average frame rate and the duration.
        let json = r#"{"streams":[
            {"codec_type":"video","width":320,"height":240,"avg_frame_rate":"10/1"}
        ],"format":{"duration":"1.500000"}}"#;
        let probe = parse_ffprobe_json(json).expect("valid ffprobe output parses");
        assert!(probe.has_video && !probe.has_audio);
        assert_eq!(probe.frames, Some(15));
        // Garbage and duration-less output do not parse.
        assert!(parse_ffprobe_json("not json").is_none());
        assert!(parse_ffprobe_json(r#"{"streams":[]}"#).is_none());
    }

    #[test]
    fn audio_stats_measures_peak_rms_and_silence() {
        let (peak, rms) = audio_stats(&[0.5, -0.5, 0.5, -0.5]);
        assert_eq!(peak, 0.5);
        assert!((rms - 0.5).abs() < 1e-6);
        assert_eq!(audio_stats(&[0.0; 8]), (0.0, 0.0));
        assert_eq!(audio_stats(&[]), (0.0, 0.0));
        assert_eq!(fmt_db(1.0), "0.0 dB");
        assert_eq!(fmt_db(0.5), "-6.0 dB");
        assert_eq!(fmt_db(0.0), "-inf dB");
    }

    #[test]
    fn band_ink_counts_text_colour_in_the_anchored_band_only() {
        // 8×8 frame, black except a white block in the
        // bottom band and one white pixel at the top.
        let (w, h) = (8usize, 8usize);
        let mut rgb = vec![0u8; w * h * 3];
        for y in 6..8 {
            for x in 2..6 {
                let i = (y * w + x) * 3;
                rgb[i] = 255;
                rgb[i + 1] = 255;
                rgb[i + 2] = 255;
            }
        }
        let i = (0 * w + 0) * 3;
        rgb[i] = 255;
        rgb[i + 1] = 255;
        rgb[i + 2] = 255;
        assert_eq!(band_ink_count(&rgb, w, h, false, [255, 255, 255]), 8);
        assert_eq!(band_ink_count(&rgb, w, h, true, [255, 255, 255]), 1);
        assert_eq!(band_ink_count(&rgb, w, h, false, [255, 0, 0]), 0);
        assert_eq!(band_ink_count(&[], w, h, false, [255, 255, 255]), 0);
    }

    #[test]
    fn qa_verdict_passes_a_healthy_measurement_set() {
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &healthy_measurements());
        assert!(report.passed, "failures: {:?}", report.failures);
        assert!(report.failures.is_empty());
        assert!(report.details.contains("48 frames"), "{}", report.details);
        assert!(report.details.contains("no subtitles"), "{}", report.details);
    }

    #[test]
    fn qa_verdict_fails_loudly_on_silent_audio() {
        // The P0-1 regression shape at the detector
        // level: the export claims the project's audio,
        // the file has a stream, and the stream is
        // digital silence.
        let mut m = healthy_measurements();
        m.audio = Some((0.0, 0.0));
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &m);
        assert!(!report.passed);
        assert_eq!(report.failures.len(), 1);
        assert!(report.failures[0].contains("silent"), "{}", report.failures[0]);
        // A missing stream is its own named failure.
        let mut m = healthy_measurements();
        m.probe.as_mut().expect("probe").has_audio = false;
        m.audio = None;
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &m);
        assert!(!report.passed);
        assert!(report.failures[0].contains("missing"), "{}", report.failures[0]);
    }

    #[test]
    fn qa_verdict_accepts_absent_audio_when_none_is_expected() {
        // The full WebM's documented shape: no audio
        // stream, none expected — not a failure.
        let mut m = healthy_measurements();
        m.probe.as_mut().expect("probe").has_audio = false;
        m.audio = None;
        let mut expect = expect_with_audio();
        expect.audio_expected = false;
        let report = qa_verdict("./export/x.webm", &expect, &m);
        assert!(report.passed, "failures: {:?}", report.failures);
    }

    #[test]
    fn qa_verdict_catches_duration_frame_and_black_mismatches() {
        let mut m = healthy_measurements();
        m.probe.as_mut().expect("probe").duration_secs = 5.0;
        m.probe.as_mut().expect("probe").frames = Some(20);
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &m);
        assert!(!report.passed);
        assert_eq!(report.failures.len(), 2, "{:?}", report.failures);
        // A partly black video passes the black scan —
        // fades are legal; only ALL sampled frames black
        // fails.
        let mut m = healthy_measurements();
        m.lumas = vec![0.0, 0.0, 0.4, 0.0];
        assert!(qa_verdict("./export/x.mp4", &expect_with_audio(), &m).passed);
        m.lumas = vec![0.0, 0.01, 0.0, 0.02];
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &m);
        assert!(!report.passed);
        assert!(report.failures[0].contains("black"), "{}", report.failures[0]);
        // An unprobeable file fails with that one fact.
        let report = qa_verdict("./export/x.mp4", &expect_with_audio(), &QaMeasurements::default());
        assert!(!report.passed);
        assert_eq!(report.failures.len(), 1);
    }

    #[test]
    fn qa_verdict_catches_a_missing_burn_in() {
        let mut expect = expect_with_audio();
        expect.audio_expected = false;
        expect.burn_in = Some(QaBurnIn {
            at_secs: 1.0,
            text_rgb: [255, 255, 255],
            position: stykanimate_rs::subtitles::SubtitlePosition::Bottom,
        });
        let mut m = healthy_measurements();
        m.ink = Some(0);
        let report = qa_verdict("./export/x.mp4", &expect, &m);
        assert!(!report.passed);
        assert!(report.failures[0].contains("burn-in"), "{}", report.failures[0]);
        m.ink = Some(QA_INK_MIN);
        assert!(qa_verdict("./export/x.mp4", &expect, &m).passed);
        // The check itself failing to run is reported,
        // not silently passed.
        m.ink = None;
        let report = qa_verdict("./export/x.mp4", &expect, &m);
        assert!(!report.passed);
    }

    #[test]
    fn qa_expectations_read_the_project() {
        let mut core = EditorCore::new();
        // The demo project: no clips, no cues.
        let expect = qa_expectations(&core.project, None, true);
        assert!(!expect.audio_expected);
        assert!(expect.burn_in.is_none());
        assert_eq!(
            expect.frames,
            Some(stykanimate_rs::export::sampled_frame_total(&core.project))
        );
        assert!(expect.duration_secs > 0.0);
        // A cue plants the spot-check at its midpoint,
        // in the style's colour, at the style's anchor.
        core.project.subtitles.push(stykanimate_rs::subtitles::SubtitleCue {
            start_frame: 0,
            end_frame: 2,
            text: "Hello".to_string(),
            speaker: None,
        });
        let expect = qa_expectations(&core.project, None, true);
        let burn = expect.burn_in.expect("cue expected");
        assert_eq!(burn.text_rgb, [255, 255, 255]);
        assert_eq!(
            burn.at_secs,
            core.project.seconds_at_frame_time(1.0),
        );
        // An unmuted clip whose file exists expects
        // audio; muting it withdraws the expectation,
        // and the video-only door never expects any.
        let clip_path = qa_temp_path("clip", "wav");
        std::fs::write(&clip_path, b"placeholder").expect("temp clip");
        core.project.audio.push(stykanimate_rs::model::AudioClip {
            path: clip_path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        assert!(qa_expectations(&core.project, None, true).audio_expected);
        assert!(!qa_expectations(&core.project, None, false).audio_expected);
        core.project.audio[0].muted = true;
        assert!(!qa_expectations(&core.project, None, true).audio_expected);
        let _ = std::fs::remove_file(&clip_path);
    }

    fn qa_fixture_dir(tag: &str) -> PathBuf {
        let dir = qa_temp_path(tag, "dir");
        std::fs::create_dir_all(&dir).expect("fixture dir");
        dir
    }

    /// A one-second lavfi MP4 with the given audio
    /// source — the probe family's real-file fixture
    /// (RM1's precedent: CI installs ffmpeg).
    fn lavfi_mp4(tag: &str, audio_source: &str) -> PathBuf {
        let out = qa_fixture_dir(tag).join("probe.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc2=duration=1:size=320x240:rate=10")
            .args(["-f", "lavfi", "-i"])
            .arg(audio_source)
            .args(["-t", "1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
            .arg(&out)
            .status()
            .expect("ffmpeg is installed in CI");
        assert!(status.success(), "fixture encode failed");
        out
    }

    #[test]
    fn probe_export_passes_a_real_healthy_mp4() {
        let path = lavfi_mp4("healthy", "sine=frequency=440:duration=1");
        let expect = QaExpectations {
            duration_secs: 1.0,
            frames: Some(10),
            audio_expected: true,
            burn_in: None,
        };
        let report = probe_export(&path, &expect);
        assert!(report.passed, "failures: {:?}", report.failures);
    }

    #[test]
    fn probe_export_fails_the_planted_silent_audio_fixture() {
        // Phase 2 acceptance: the QA recap fails loudly
        // on a planted silent-audio file — an audio
        // stream is present, the project audio is
        // expected, and the stream meters at silence.
        let path = lavfi_mp4("silent", "anullsrc=channel_layout=stereo:sample_rate=44100");
        let expect = QaExpectations {
            duration_secs: 1.0,
            frames: Some(10),
            audio_expected: true,
            burn_in: None,
        };
        let report = probe_export(&path, &expect);
        assert!(!report.passed);
        assert!(
            report.failures.iter().any(|f| f.contains("silent")),
            "failures: {:?}",
            report.failures
        );
        // The same file passes when no audio is
        // expected of it (the video-only shape).
        let mut relaxed = expect.clone();
        relaxed.audio_expected = false;
        assert!(probe_export(&path, &relaxed).passed);
    }

    #[test]
    fn probe_export_spots_burned_in_text_and_its_absence() {
        // A white bar in the bottom band stands in for
        // burned-in text: the ink counter must find it,
        // and must find nothing on the plain black file.
        let burn = QaBurnIn {
            at_secs: 0.5,
            text_rgb: [255, 255, 255],
            position: stykanimate_rs::subtitles::SubtitlePosition::Bottom,
        };
        let expect = QaExpectations {
            duration_secs: 1.0,
            frames: Some(10),
            audio_expected: false,
            burn_in: Some(burn),
        };
        let with_text = qa_fixture_dir("burnin").join("with.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("color=c=black:s=320x240:d=1:r=10")
            .args(["-vf", "drawbox=x=40:y=196:w=240:h=20:color=white:t=fill"])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&with_text)
            .status()
            .expect("ffmpeg is installed in CI");
        assert!(status.success());
        let report = probe_export(&with_text, &expect);
        assert!(
            !report.failures.iter().any(|f| f.contains("burn-in")),
            "failures: {:?}",
            report.failures
        );
        let plain = qa_fixture_dir("noburn").join("plain.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("color=c=black:s=320x240:d=1:r=10")
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&plain)
            .status()
            .expect("ffmpeg is installed in CI");
        assert!(status.success());
        let report = probe_export(&plain, &expect);
        assert!(
            report.failures.iter().any(|f| f.contains("burn-in missing")),
            "failures: {:?}",
            report.failures
        );
    }

    // ---- Episode QA probe (Roadmap Wave 2, W2-20 / P-16) ----

    fn episode_check_named<'a>(report: &'a EpisodeQaReport, name: &str) -> &'a EpisodeCheck {
        report
            .checks
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no {name} check in {report:?}"))
    }

    fn episode_expectations_fixture() -> EpisodeExpectations {
        EpisodeExpectations {
            qa: QaExpectations {
                duration_secs: 6.0,
                frames: Some(144),
                audio_expected: true,
                burn_in: None,
            },
            fps: 24.0,
            total_frames: 144,
            chapters: vec![
                EpisodeChapterExpectation {
                    title: "Opening".to_string(),
                    start_secs: 0.0,
                    end_secs: 3.0,
                },
                EpisodeChapterExpectation {
                    title: "Finale".to_string(),
                    start_secs: 3.0,
                    end_secs: 6.0,
                },
            ],
            title_cards_enabled: true,
            cards: vec![
                EpisodeCardExpectation {
                    chapter: 0,
                    title: "Opening".to_string(),
                    at_secs: 1.25,
                    text_rgb: [255, 255, 255],
                },
                EpisodeCardExpectation {
                    chapter: 1,
                    title: "Finale".to_string(),
                    at_secs: 4.25,
                    text_rgb: [255, 255, 255],
                },
            ],
            cue_facts: vec![EpisodeCueFact {
                index: 0,
                start_frame: 24,
                end_frame: 120,
                chars: 19,
                cps: 4.75,
            }],
            cue_targets: vec![0],
        }
    }

    fn episode_measurements_healthy() -> EpisodeMeasurements {
        EpisodeMeasurements {
            probe: Some(VideoProbe {
                duration_secs: 6.0,
                has_video: true,
                has_audio: true,
                frames: Some(144),
                width: 320,
                height: 180,
            }),
            audio_scan: Some(EpisodeAudioScan {
                peak: 0.5,
                rms: 0.35,
                silence_stretches: vec![(2.0, 2.5)],
            }),
            frame_samples: (0..12)
                .map(|i| EpisodeFrameSample {
                    at_secs: 0.25 + 0.5 * i as f32,
                    luma: 0.3,
                    fingerprint: 1000 + i as u64,
                })
                .collect(),
            frame_interval_secs: 0.5,
            file_chapters: Some(vec![
                EpisodeFileChapter {
                    title: "Opening".to_string(),
                    start_secs: 0.0,
                    end_secs: 3.0,
                },
                EpisodeFileChapter {
                    title: "Finale".to_string(),
                    start_secs: 3.0,
                    end_secs: 6.0,
                },
            ]),
            cue_ink: vec![Some(500)],
            card_ink: vec![Some(900), Some(900)],
        }
    }

    #[test]
    fn episode_verdict_passes_a_healthy_episode() {
        let report = episode_qa_verdict(
            "./export/ep.mp4",
            &episode_expectations_fixture(),
            &episode_measurements_healthy(),
            &EpisodeQaParams::default(),
        );
        assert!(report.passed, "checks: {:?}", report.checks);
        assert!(
            report.checks.iter().all(|c| c.status == EpisodeCheckStatus::Pass),
            "checks: {:?}",
            report.checks
        );
        assert!(report.summary.contains("0 failed"), "{}", report.summary);
        assert!(report.summary.contains("0 unmeasured"), "{}", report.summary);
    }

    #[test]
    fn episode_verdict_names_each_planted_defect() {
        let params = EpisodeQaParams::default();
        let expect = episode_expectations_fixture();
        // A silent stretch inside a sounding mix: the
        // level check still passes (the peak is real),
        // the stretch check fails by name and span.
        let mut m = episode_measurements_healthy();
        m.audio_scan.as_mut().expect("scan").silence_stretches = vec![(2.0, 6.0)];
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        assert!(!report.passed);
        assert_eq!(
            episode_check_named(&report, "audio").status,
            EpisodeCheckStatus::Pass
        );
        let stretch = episode_check_named(&report, "silence-stretches");
        assert_eq!(stretch.status, EpisodeCheckStatus::Fail);
        assert!(
            stretch.evidence.contains("silent from 2.0s to 6.0s"),
            "{}",
            stretch.evidence
        );
        // A black stretch: samples 6..12 dark (3.0s–6.0s).
        let mut m = episode_measurements_healthy();
        for s in &mut m.frame_samples[6..] {
            s.luma = 0.0;
        }
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        let black = episode_check_named(&report, "black-stretches");
        assert_eq!(black.status, EpisodeCheckStatus::Fail);
        assert!(
            black.evidence.contains("black frames from 3.0s to 6.0s"),
            "{}",
            black.evidence
        );
        assert_eq!(
            episode_check_named(&report, "frozen-stretches").status,
            EpisodeCheckStatus::Pass
        );
        // A frozen stretch: samples 4..12 identical
        // (2.0s–6.0s) under a lit picture.
        let mut m = episode_measurements_healthy();
        for s in &mut m.frame_samples[4..] {
            s.fingerprint = 42;
        }
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        let frozen = episode_check_named(&report, "frozen-stretches");
        assert_eq!(frozen.status, EpisodeCheckStatus::Fail);
        assert!(
            frozen.evidence.contains("frozen from 2.0s to 6.0s"),
            "{}",
            frozen.evidence
        );
        // Missing chapter markers.
        let mut m = episode_measurements_healthy();
        m.file_chapters = Some(Vec::new());
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        let chapters = episode_check_named(&report, "chapters");
        assert_eq!(chapters.status, EpisodeCheckStatus::Fail);
        assert!(
            chapters.evidence.contains("0 chapter markers in the file, 2 expected"),
            "{}",
            chapters.evidence
        );
        // A mistitled marker fails on the title, a
        // mistimed one on the start.
        let mut m = episode_measurements_healthy();
        m.file_chapters.as_mut().expect("chapters")[1].title = "End".to_string();
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        let chapters = episode_check_named(&report, "chapters");
        assert_eq!(chapters.status, EpisodeCheckStatus::Fail);
        assert!(chapters.evidence.contains("\"End\""), "{}", chapters.evidence);
        // A missing title card, named by chapter.
        let mut m = episode_measurements_healthy();
        m.card_ink = vec![Some(0), Some(900)];
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        let cards = episode_check_named(&report, "title-cards");
        assert_eq!(cards.status, EpisodeCheckStatus::Fail);
        assert!(
            cards.evidence.contains("chapter 1 (\"Opening\")"),
            "{}",
            cards.evidence
        );
        // A missing burn-in at the spot-checked cue.
        let mut m = episode_measurements_healthy();
        m.cue_ink = vec![Some(0)];
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        assert_eq!(
            episode_check_named(&report, "subtitles").status,
            EpisodeCheckStatus::Fail
        );
        // Duration and frame-count drift.
        let mut m = episode_measurements_healthy();
        m.probe.as_mut().expect("probe").duration_secs = 9.0;
        m.probe.as_mut().expect("probe").frames = Some(20);
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        assert_eq!(
            episode_check_named(&report, "duration").status,
            EpisodeCheckStatus::Fail
        );
        assert_eq!(
            episode_check_named(&report, "frame-count").status,
            EpisodeCheckStatus::Fail
        );
    }

    #[test]
    fn episode_verdict_reports_reading_speed_without_judging_it() {
        // D-9: a very fast cue is a reported fact, never
        // a verdict — the roadmap has set no threshold.
        let mut expect = episode_expectations_fixture();
        expect.cue_facts[0].cps = 42.0;
        expect.cue_facts[0].chars = 168;
        let report = episode_qa_verdict(
            "./ep.mp4",
            &expect,
            &episode_measurements_healthy(),
            &EpisodeQaParams::default(),
        );
        let timing = episode_check_named(&report, "cue-timing");
        assert_eq!(timing.status, EpisodeCheckStatus::Pass);
        assert!(timing.evidence.contains("42.0 chars/s"), "{}", timing.evidence);
        assert!(timing.evidence.contains("not judged"), "{}", timing.evidence);
        assert!(report.passed);
    }

    #[test]
    fn episode_verdict_catches_a_cue_past_the_episode_end() {
        let mut expect = episode_expectations_fixture();
        expect.cue_facts[0].end_frame = 200;
        let report = episode_qa_verdict(
            "./ep.mp4",
            &expect,
            &episode_measurements_healthy(),
            &EpisodeQaParams::default(),
        );
        let timing = episode_check_named(&report, "cue-timing");
        assert_eq!(timing.status, EpisodeCheckStatus::Fail);
        assert!(
            timing.evidence.contains("runs past the episode end"),
            "{}",
            timing.evidence
        );
        assert!(!report.passed);
    }

    #[test]
    fn episode_verdict_unmeasured_is_first_class() {
        let params = EpisodeQaParams::default();
        let expect = episode_expectations_fixture();
        // Decodes that never landed: the level check
        // fails (audio was expected — RM29's rule),
        // everything downstream of the missing
        // measurements is Unmeasured, not passed.
        let mut m = episode_measurements_healthy();
        m.audio_scan = None;
        m.frame_samples = Vec::new();
        m.file_chapters = None;
        m.cue_ink = vec![None];
        m.card_ink = vec![None, None];
        let report = episode_qa_verdict("./ep.mp4", &expect, &m, &params);
        assert_eq!(
            episode_check_named(&report, "audio").status,
            EpisodeCheckStatus::Fail
        );
        for name in [
            "silence-stretches",
            "black-stretches",
            "frozen-stretches",
            "subtitles",
            "title-cards",
            "chapters",
        ] {
            assert_eq!(
                episode_check_named(&report, name).status,
                EpisodeCheckStatus::Unmeasured,
                "{name} should be Unmeasured"
            );
        }
        // An unprobeable file: one probe failure, the
        // file checks Unmeasured, and the project-side
        // cue-timing check still judges.
        let report = episode_qa_verdict(
            "./ep.mp4",
            &expect,
            &EpisodeMeasurements::default(),
            &params,
        );
        assert!(!report.passed);
        assert_eq!(
            episode_check_named(&report, "probe").status,
            EpisodeCheckStatus::Fail
        );
        assert_eq!(
            episode_check_named(&report, "duration").status,
            EpisodeCheckStatus::Unmeasured
        );
        assert_eq!(
            episode_check_named(&report, "cue-timing").status,
            EpisodeCheckStatus::Pass
        );
    }

    #[test]
    fn episode_audio_scan_finds_stretches_and_meters() {
        // 1s of tone, 3s of digital silence, 1s of tone
        // at a 1 kHz rate (50-sample windows).
        let mut samples = vec![0.5f32; 1000];
        samples.extend(vec![0.0f32; 3000]);
        samples.extend(vec![0.5f32; 1000]);
        let scan = audio_scan(&samples, 1000, QA_SILENCE_PEAK);
        assert_eq!(scan.peak, 0.5);
        assert!((scan.rms - (0.1f32).sqrt()).abs() < 1e-5, "{}", scan.rms);
        assert_eq!(scan.silence_stretches.len(), 1);
        let (a, b) = scan.silence_stretches[0];
        assert!((a - 1.0).abs() < 0.06 && (b - 4.0).abs() < 0.06, "{a}..{b}");
        // A wholly silent file is one stretch from 0.
        let scan = audio_scan(&vec![0.0f32; 2000], 1000, QA_SILENCE_PEAK);
        assert_eq!(scan.silence_stretches.len(), 1);
        let (a, b) = scan.silence_stretches[0];
        assert!(a == 0.0 && (b - 2.0).abs() < 0.06, "{a}..{b}");
        // No samples, no stretches — absence, not a pass
        // manufactured from nothing.
        let scan = audio_scan(&[], 44100, QA_SILENCE_PEAK);
        assert!(scan.silence_stretches.is_empty());
        assert_eq!(scan.peak, 0.0);
    }

    #[test]
    fn episode_ffmetadata_parse_round_trips_titles_and_times() {
        let mut project = stykanimate_rs::model::Project::new_demo();
        project.fps = 24.0;
        let frame = project.frames[0].clone();
        project.frames = vec![frame; 144];
        project.chapters = vec![
            stykanimate_rs::model::Chapter {
                title: "A; B=C #D \\ E".to_string(),
                start_frame: 0,
                end_frame: 72,
            },
            stykanimate_rs::model::Chapter {
                title: "Finale".to_string(),
                start_frame: 72,
                end_frame: 144,
            },
        ];
        let doc = stykanimate_rs::export::ffmetadata_chapters(&project)
            .expect("chapters resolve to a document");
        let parsed = parse_ffmetadata_chapters(&doc);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].title, "A; B=C #D \\ E");
        assert_eq!((parsed[0].start_secs, parsed[0].end_secs), (0.0, 3.0));
        assert_eq!(parsed[1].title, "Finale");
        assert_eq!((parsed[1].start_secs, parsed[1].end_secs), (3.0, 6.0));
        assert!(parse_ffmetadata_chapters(";FFMETADATA1\n").is_empty());
    }

    #[test]
    fn episode_ffprobe_chapters_json_parses() {
        let json = r#"{"chapters":[
            {"id":0,"time_base":"1/1000","start":0,"start_time":"0.000000","end":3000,"end_time":"3.000000","tags":{"title":"Opening"}},
            {"id":1,"time_base":"1/1000","start":3000,"start_time":"3.000000","end":6000,"end_time":"6.000000","tags":{"title":"Finale"}}
        ]}"#;
        let chapters = parse_ffprobe_chapters_json(json).expect("valid output parses");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "Opening");
        assert_eq!((chapters[1].start_secs, chapters[1].end_secs), (3.0, 6.0));
        // A chapterless file parses to an empty list —
        // absence of markers is a measurement, not an
        // error.
        assert_eq!(
            parse_ffprobe_chapters_json(r#"{"chapters":[]}"#),
            Some(Vec::new())
        );
        assert!(parse_ffprobe_chapters_json("not json").is_none());
    }

    #[test]
    fn episode_centre_ink_counts_the_middle_band_only() {
        // 8×8 frame: white in the centre band counts,
        // white at the top and bottom edges does not.
        let (w, h) = (8usize, 8usize);
        let mut rgb = vec![0u8; w * h * 3];
        let mut paint = |rgb: &mut Vec<u8>, x: usize, y: usize| {
            let i = (y * w + x) * 3;
            rgb[i] = 255;
            rgb[i + 1] = 255;
            rgb[i + 2] = 255;
        };
        for y in 3..5 {
            for x in 2..6 {
                paint(&mut rgb, x, y);
            }
        }
        paint(&mut rgb, 0, 0);
        paint(&mut rgb, 7, 7);
        assert_eq!(centre_ink_count(&rgb, w, h, [255, 255, 255]), 8);
        assert_eq!(centre_ink_count(&rgb, w, h, [255, 0, 0]), 0);
        assert_eq!(centre_ink_count(&[], w, h, [255, 255, 255]), 0);
    }

    /// The fixture project: 144 frames at 24 fps (a 6s
    /// episode), two chapters, title cards on at the
    /// 2.5s default, one cue over frames 24–120, and an
    /// unmuted clip whose file exists so audio is
    /// expected (RM29's placeholder-file precedent).
    /// Returns the project and the clip path to clean
    /// up.
    fn episode_project() -> (stykanimate_rs::model::Project, PathBuf) {
        let mut project = stykanimate_rs::model::Project::new_demo();
        project.fps = 24.0;
        let frame = project.frames[0].clone();
        project.frames = vec![frame; 144];
        project.chapters = vec![
            stykanimate_rs::model::Chapter {
                title: "Opening".to_string(),
                start_frame: 0,
                end_frame: 72,
            },
            stykanimate_rs::model::Chapter {
                title: "Finale".to_string(),
                start_frame: 72,
                end_frame: 144,
            },
        ];
        project.title_cards.enabled = true;
        project.subtitles = vec![stykanimate_rs::subtitles::SubtitleCue {
            start_frame: 24,
            end_frame: 120,
            text: "Hello episode world".to_string(),
            speaker: None,
        }];
        let clip_path = qa_temp_path("epclip", "wav");
        std::fs::write(&clip_path, b"placeholder").expect("temp clip");
        project.audio.push(stykanimate_rs::model::AudioClip {
            path: clip_path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        (project, clip_path)
    }

    #[test]
    fn episode_expectations_read_the_project() {
        let (project, clip_path) = episode_project();
        let expect = episode_expectations(&project);
        assert!(expect.qa.audio_expected);
        assert_eq!(expect.total_frames, 144);
        assert_eq!(expect.qa.duration_secs, 6.0);
        assert_eq!(
            expect.qa.frames,
            Some(stykanimate_rs::export::sampled_frame_total(&project))
        );
        // Chapters resolve through the exporter's own
        // document: ms on the authored clock.
        assert_eq!(expect.chapters.len(), 2);
        assert_eq!(expect.chapters[0].title, "Opening");
        assert_eq!(
            (expect.chapters[1].start_secs, expect.chapters[1].end_secs),
            (3.0, 6.0)
        );
        // Card targets sit at the windows' middle
        // frames: [0,60) → frame 30 → 1.25s, [72,132) →
        // frame 102 → 4.25s.
        assert!(expect.title_cards_enabled);
        assert_eq!(expect.cards.len(), 2);
        assert_eq!(expect.cards[0].at_secs, 1.25);
        assert_eq!(expect.cards[1].at_secs, 4.25);
        assert_eq!(expect.cards[0].text_rgb, [255, 255, 255]);
        // The cue fact: 19 chars over 96 frames (4.0s).
        assert_eq!(expect.cue_facts.len(), 1);
        assert_eq!(expect.cue_facts[0].chars, 19);
        assert!((expect.cue_facts[0].cps - 4.75).abs() < 1e-4);
        assert_eq!(expect.cue_targets, vec![0]);
        // Cards off: no targets, and the flag says why.
        let (mut off, _) = episode_project();
        off.title_cards.enabled = false;
        let expect = episode_expectations(&off);
        assert!(!expect.title_cards_enabled);
        assert!(expect.cards.is_empty());
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_cue_coverage_merges_overlaps_and_clamps() {
        let mut expect = episode_expectations_fixture();
        expect.cue_facts = vec![
            EpisodeCueFact {
                index: 0,
                start_frame: 0,
                end_frame: 72,
                chars: 1,
                cps: 0.0,
            },
            EpisodeCueFact {
                index: 1,
                start_frame: 48,
                end_frame: 200,
                chars: 1,
                cps: 0.0,
            },
        ];
        // [0,72) ∪ [48,144 clamped) = the whole episode.
        assert_eq!(episode_cue_coverage(&expect), 1.0);
        expect.cue_facts.truncate(1);
        assert_eq!(episode_cue_coverage(&expect), 0.5);
    }

    /// Run ffmpeg over a fixture argument list (RM29's
    /// real-file precedent: CI installs ffmpeg).
    fn episode_ffmpeg(args: &[&str]) {
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y"])
            .args(args)
            .status()
            .expect("ffmpeg is installed in CI");
        assert!(status.success(), "fixture encode failed: {args:?}");
    }

    /// The moving patch: a solid mid-tone square riding
    /// an overlay path over a dark blue-grey ground —
    /// no near-white anywhere (so ink counts read only
    /// the planted bars), never black, never still.
    const EP_MOVING: &str =
        "[0:v][1:v]overlay=x='100+60*sin(t*1.7)':y='60+30*cos(t*1.1)'[bg]";
    /// The good episode's planted bars: white card bars
    /// in the centre band over both card windows
    /// (0–2.5s, 3–5.5s) and a white cue bar in the
    /// bottom band over the cue's span (1–5s).
    const EP_BOXES: &str = "[bg]drawbox=x=60:y=76:w=200:h=28:color=white:t=fill:enable='between(t,0,2.5)',drawbox=x=60:y=76:w=200:h=28:color=white:t=fill:enable='between(t,3,5.5)',drawbox=x=40:y=146:w=240:h=20:color=white:t=fill:enable='between(t,1,5)'[v]";
    /// The same episode with the card bars never drawn.
    const EP_BOXES_NO_CARD: &str =
        "[bg]drawbox=x=40:y=146:w=240:h=20:color=white:t=fill:enable='between(t,1,5)'[v]";

    /// Plant the chapter markers into a fixture base
    /// the way the app's own embed does (a stream-copy
    /// remux against an FFMETADATA document).
    fn episode_remux_chapters(dir: &Path, base: &Path) -> PathBuf {
        let meta = dir.join("chapters.txt");
        std::fs::write(
            &meta,
            ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=3000\ntitle=Opening\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=3000\nEND=6000\ntitle=Finale\n",
        )
        .expect("chapter document");
        let out = dir.join("episode.mp4");
        episode_ffmpeg(&[
            "-i",
            base.to_str().expect("base path"),
            "-i",
            meta.to_str().expect("meta path"),
            "-map",
            "0",
            "-map_chapters",
            "1",
            "-c",
            "copy",
            out.to_str().expect("out path"),
        ]);
        out
    }

    /// The synthetic good episode (or a sibling with
    /// one planted defect): 6s at 24 fps, 320×180, a
    /// 440 Hz tone throughout — optionally silenced
    /// from 2s on — and optionally chapter-marked.
    fn episode_fixture(
        tag: &str,
        boxes: &str,
        silent_stretch: bool,
        chapters: bool,
    ) -> PathBuf {
        let dir = qa_fixture_dir(tag);
        let base = dir.join("base.mp4");
        let audio = if silent_stretch {
            "[2:a]volume=enable='gte(t,2)':volume=0[a]"
        } else {
            "[2:a]anull[a]"
        };
        let graph = format!("{EP_MOVING};{boxes};{audio}");
        episode_ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x203040:size=320x180:duration=6:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x907040:size=60x40:duration=6:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-filter_complex",
            graph.as_str(),
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            base.to_str().expect("base path"),
        ]);
        if chapters {
            episode_remux_chapters(&dir, &base)
        } else {
            base
        }
    }

    #[test]
    fn episode_probe_passes_the_synthetic_good_episode() {
        let (project, clip_path) = episode_project();
        let path = episode_fixture("epgood", EP_BOXES, false, true);
        let report = probe_episode(&path, &project, &EpisodeQaParams::default());
        assert!(report.passed, "checks: {:?}", report.checks);
        assert!(
            report.checks.iter().all(|c| c.status == EpisodeCheckStatus::Pass),
            "checks: {:?}",
            report.checks
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_the_planted_silent_stretch() {
        let (project, clip_path) = episode_project();
        let path = episode_fixture("epsilent", EP_BOXES, true, true);
        let report = probe_episode(&path, &project, &EpisodeQaParams::default());
        assert!(!report.passed);
        // The mix sounds (its peak is real) — only the
        // stretch is the defect.
        assert_eq!(
            episode_check_named(&report, "audio").status,
            EpisodeCheckStatus::Pass
        );
        let stretch = episode_check_named(&report, "silence-stretches");
        assert_eq!(stretch.status, EpisodeCheckStatus::Fail);
        assert!(
            stretch.evidence.contains("audio silent from 2."),
            "{}",
            stretch.evidence
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_the_planted_black_stretch() {
        let (project, clip_path) = episode_project();
        let dir = qa_fixture_dir("epblack");
        let out = dir.join("episode.mp4");
        episode_ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x203040:size=320x180:duration=3:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x907040:size=60x40:duration=3:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:size=320x180:duration=3:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-filter_complex",
            "[0:v][1:v]overlay=x='100+60*sin(t*1.7)':y='60+30*cos(t*1.1)'[a];[a][2:v]concat=n=2:v=1:a=0[v];[3:a]anull[a]",
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            out.to_str().expect("out path"),
        ]);
        let report = probe_episode(&out, &project, &EpisodeQaParams::default());
        let black = episode_check_named(&report, "black-stretches");
        assert_eq!(black.status, EpisodeCheckStatus::Fail, "{report:?}");
        assert!(
            black.evidence.contains("black frames from"),
            "{}",
            black.evidence
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_the_planted_frozen_stretch() {
        let (project, clip_path) = episode_project();
        let dir = qa_fixture_dir("epfrozen");
        let out = dir.join("episode.mp4");
        episode_ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x203040:size=320x180:duration=2:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x907040:size=60x40:duration=2:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x203040:size=320x180:duration=4:rate=24",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x907040:size=60x40:duration=4:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-filter_complex",
            "[0:v][1:v]overlay=x='100+60*sin(t*1.7)':y='60+30*cos(t*1.1)'[a];[2:v][3:v]overlay=x=120:y=70[b];[a][b]concat=n=2:v=1:a=0[v];[4:a]anull[a]",
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            out.to_str().expect("out path"),
        ]);
        let report = probe_episode(&out, &project, &EpisodeQaParams::default());
        let frozen = episode_check_named(&report, "frozen-stretches");
        assert_eq!(frozen.status, EpisodeCheckStatus::Fail, "{report:?}");
        assert!(
            frozen.evidence.contains("picture frozen from"),
            "{}",
            frozen.evidence
        );
        // The frozen picture is lit — the black check is
        // not the one that fires.
        assert_eq!(
            episode_check_named(&report, "black-stretches").status,
            EpisodeCheckStatus::Pass
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_missing_chapter_markers() {
        let (project, clip_path) = episode_project();
        let path = episode_fixture("epnochapters", EP_BOXES, false, false);
        let report = probe_episode(&path, &project, &EpisodeQaParams::default());
        let chapters = episode_check_named(&report, "chapters");
        assert_eq!(chapters.status, EpisodeCheckStatus::Fail);
        assert!(
            chapters.evidence.contains("0 chapter markers in the file, 2 expected"),
            "{}",
            chapters.evidence
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_a_missing_title_card() {
        let (project, clip_path) = episode_project();
        let path = episode_fixture("epnocard", EP_BOXES_NO_CARD, false, true);
        let report = probe_episode(&path, &project, &EpisodeQaParams::default());
        let cards = episode_check_named(&report, "title-cards");
        assert_eq!(cards.status, EpisodeCheckStatus::Fail, "{report:?}");
        assert!(
            cards.evidence.contains("title card missing for chapter 1"),
            "{}",
            cards.evidence
        );
        // The cue bar is still drawn — subtitles pass.
        assert_eq!(
            episode_check_named(&report, "subtitles").status,
            EpisodeCheckStatus::Pass
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn episode_probe_catches_a_cue_past_the_episode_end() {
        let (mut project, clip_path) = episode_project();
        project.subtitles[0].end_frame = 200;
        let path = episode_fixture("epcuepast", EP_BOXES, false, true);
        let report = probe_episode(&path, &project, &EpisodeQaParams::default());
        let timing = episode_check_named(&report, "cue-timing");
        assert_eq!(timing.status, EpisodeCheckStatus::Fail);
        assert!(
            timing.evidence.contains("runs past the episode end"),
            "{}",
            timing.evidence
        );
        let _ = std::fs::remove_file(&clip_path);
    }

    #[test]
    fn sequence_residue_counts_only_frame_files() {
        let dir = qa_fixture_dir("residue");
        for name in ["frame-0001.png", "frame-0002.png", "frame-0010.png"] {
            std::fs::write(dir.join(name), b"x").expect("frame file");
        }
        // A hand-named file and another format's master
        // are not residue.
        std::fs::write(dir.join("frame-draft.png"), b"x").expect("draft");
        std::fs::write(dir.join("untitled.mp4"), b"x").expect("master");
        assert_eq!(sequence_residue(&dir), 3);
        assert_eq!(sequence_residue(&dir.join("missing")), 0);
    }

    #[test]
    fn incomplete_rows_are_marked_from_the_registry() {
        // The registry names the row the PNG jobs write
        // ("export/frame-*.png"); a directory holding
        // fewer frames than expected marks the row, a
        // full one does not, and the mark shows on the
        // row's line only when set.
        note_expected_sequence("export/frame-*.png", 5);
        let triples: Vec<FileTriple> = (1..=3)
            .map(|i| {
                (
                    PathBuf::from(format!("export/frame-{i:04}.png")),
                    100,
                    at(500),
                    1,
                )
            })
            .collect();
        let mut rows = rows_from_triples(&triples);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].incomplete, "unmarked before the registry is read");
        mark_incomplete(&mut rows);
        assert!(rows[0].incomplete);
        let core = EditorCore::new();
        let marked = row_line(&core, &rows[0], at(500));
        assert!(
            marked.ends_with(&i18n_wire::tr(&core, "export_history_incomplete")),
            "{marked}"
        );
        let mut full = rows.clone();
        full[0].count = 5;
        mark_incomplete(&mut full);
        assert!(!full[0].incomplete);
        assert_eq!(
            row_line(&core, &full[0], at(500)),
            "export/frame-*.png · 5 files · 300 B · just now"
        );
    }

    #[test]
    fn qa_labels_exist_in_the_catalogue() {
        // RM29 (IA-RM29-1): the QA verdict templates,
        // the residue line and the incomplete marker —
        // the composed panel copy of this seat. Like
        // the other IA pins in this module, this passes
        // only once the integrator inserts the keys
        // (the Q7 pattern).
        let core = EditorCore::new();
        for (key, literal) in [
            ("export_qa_pass", "QA passed — {details}"),
            ("export_qa_fail", "QA failed — {details}"),
            (
                "export_residue_line",
                "Partial export: {count} frames were written to {dest} before it failed.",
            ),
            ("export_history_incomplete", "incomplete"),
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
        // The composed lines through the public helpers:
        // a passing report renders its measured facts; a
        // failing one names its failures first.
        let pass = QaReport {
            dest: "./export/x.mp4".to_string(),
            passed: true,
            failures: Vec::new(),
            details: "2.0s · 48 frames".to_string(),
        };
        assert_eq!(qa_line(&core, &pass), "QA passed — 2.0s · 48 frames");
        let fail = QaReport {
            dest: "./export/x.mp4".to_string(),
            passed: false,
            failures: vec!["audio is silent (peak -inf dB)".to_string()],
            details: "2.0s · 48 frames".to_string(),
        };
        assert_eq!(
            qa_line(&core, &fail),
            "QA failed — audio is silent (peak -inf dB) — 2.0s · 48 frames"
        );
    }

    // ---- QA probe worker + history (W2-44) ----

    /// Drain probe reports until none are in flight —
    /// bounded, so a broken drain fails the pin instead
    /// of hanging the suite.
    fn drain_probes() {
        for _ in 0..2000 {
            poll_qa_probes();
            if !qa_probe_active() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("QA probe did not land");
    }

    #[test]
    fn queued_probe_lands_through_the_drain_not_the_finish_path() {
        // The healthy real-file fixture, probed through
        // the worker path end to end: queueing stores
        // nothing (the finish path no longer probes
        // inline); the drain lands the verdict in both
        // the per-destination reports and the history.
        let path = lavfi_mp4("worker", "sine=frequency=440:duration=1");
        let core = EditorCore::new();
        queue_probe(&core.project, &path, None, true);
        assert!(qa_probe_active());
        assert!(
            qa_reports().iter().all(|r| !r.dest.ends_with("probe.mp4")),
            "no report may be stored before the drain runs"
        );
        drain_probes();
        assert!(!qa_probe_active());
        assert!(
            qa_reports().iter().any(|r| r.dest.ends_with("probe.mp4")),
            "the drained verdict is stored"
        );
        assert!(
            qa_history()
                .iter()
                .any(|e| e.report.dest.ends_with("probe.mp4")),
            "and enters the session history"
        );
    }

    #[test]
    fn panel_constructs_while_a_probe_is_in_flight() {
        // W2-64: the in-flight probe's checking line is
        // part of the panel's construction — while a
        // probe runs, the panel builds (both themes)
        // with the line's branch taken; once the drain
        // lands the verdict, it builds with it gone.
        // The line claims a state, never a number: the
        // probe exposes no phase contract (W2-44).
        let s = crate::init();
        let path = lavfi_mp4("checking", "sine=frequency=440:duration=1");
        queue_probe(&s.core.project, &path, None, true);
        assert!(qa_probe_active());
        let _ = export_panel(&s, &rune::tok::theme(true));
        let _ = export_panel(&s, &rune::tok::theme(false));
        drain_probes();
        assert!(!qa_probe_active());
        let _ = export_panel(&s, &rune::tok::theme(true));
    }

    #[test]
    fn poll_export_job_stays_active_while_a_probe_is_in_flight() {
        // The shell-facing contract: the same tick that
        // drains the export job drains the probes, and
        // reads active until the verdict lands — so an
        // otherwise idle window keeps waking and the
        // QA line repaints when it arrives.
        let mut s = crate::init();
        let path = lavfi_mp4("poll", "sine=frequency=440:duration=1");
        queue_probe(&s.core.project, &path, None, true);
        for _ in 0..2000 {
            let (_redraw, active) = poll_export_job(&mut s);
            if !active {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            qa_reports().iter().any(|r| r.dest.ends_with("probe.mp4")),
            "the verdict landed through poll_export_job"
        );
        assert_eq!(poll_export_job(&mut s), (false, false));
    }

    #[test]
    fn dead_probe_worker_lands_a_named_failure() {
        // A probe worker that dies without sending is
        // the export worker-death case one level down:
        // the drain stores a failure naming the probe's
        // own stoppage — never a pass, never silence.
        let (_tx, rx) = mpsc::channel::<QaReport>();
        drop(_tx);
        QA_JOBS.with(|slot| {
            slot.borrow_mut().push(QaJob {
                dest: "./export/dead.mp4".to_string(),
                outcome: rx,
            })
        });
        assert!(poll_qa_probes());
        let report = qa_reports()
            .into_iter()
            .find(|r| r.dest == "./export/dead.mp4")
            .expect("the death report is stored");
        assert!(!report.passed);
        assert!(
            report.failures.iter().any(|f| f.contains("QA probe stopped")),
            "failures: {:?}",
            report.failures
        );
    }

    #[test]
    fn qa_history_caps_stamps_and_names_the_failing_check() {
        // The history keeps the newest QA_HISTORY_MAX
        // verdicts, oldest dropped; each line carries
        // the landing stamp and, on failure, the named
        // check — the history view's whole content.
        for i in 0..QA_HISTORY_MAX + 3 {
            store_report(QaReport {
                dest: format!("./export/h{i}.mp4"),
                passed: true,
                failures: Vec::new(),
                details: "1.0s · 10 frames".to_string(),
            });
        }
        let history = qa_history();
        assert_eq!(history.len(), QA_HISTORY_MAX);
        assert_eq!(history[0].report.dest, "./export/h3.mp4");
        assert_eq!(
            history[QA_HISTORY_MAX - 1].report.dest,
            format!("./export/h{}.mp4", QA_HISTORY_MAX + 2)
        );
        let core = EditorCore::new();
        let entry = QaHistoryEntry {
            report: QaReport {
                dest: "./export/x.mp4".to_string(),
                passed: false,
                failures: vec!["audio stream missing".to_string()],
                details: "2.0s · 48 frames".to_string(),
            },
            at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        };
        let line = qa_history_line(&core, &entry);
        assert!(
            line.starts_with("2023-11-14 22:13 · QA failed — audio stream missing"),
            "{line}"
        );
    }
}
