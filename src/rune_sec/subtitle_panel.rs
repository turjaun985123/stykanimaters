//! The Subtitles panel for the Rune editor shell (Wave
//! P's P31 slice, built in Wave Q by Q7) — a section at
//! the foot of the Audio workspace's panel (mounted by
//! `audio_panel`, the workspace the timeline's subtitle
//! lane is gated to show in), carrying everything the
//! subtitle core offers the GUI: the cue list, a detail
//! editor for the selected cue (range steppers, playhead
//! placement, a guarded text edit, delete), the project
//! subtitle style (position, font scale, text/box
//! colours), and SRT/WebVTT import/export through the
//! converters in `stykanimate_rs::subtitles`.
//!
//! Roadmap Wave 1 (RM21, P-11 — subtitles as citizens)
//! deepened the panel in place: the style block gained
//! the font cycler (Shaped/Pixel — IA-RM2-3, the panel
//! door RM2's burn-in routing waited on); the detail
//! block gained a time-domain readout (timecodes and
//! duration at the project frame rate), CPS/line-length
//! authoring feedback (measurement only — the stack
//! cap/wrap question is a standing §8 decision this
//! panel does not touch), a speaker line for a credited
//! cue, and nudge-by-time buttons; and a Timing block
//! (shift-all + explicit fps retiming) joined the
//! section. Promotion to a section of the panel's own
//! (out of the Audio foot) is the X9 gating move priced
//! in IA-RM21-1 — not built here.
//!
//! Roadmap Wave 2 (W2-16) adds the Timing assistant
//! block under the Timing block: RM21 measured every
//! cue's reading speed and judged nothing; the
//! assistant proposes concrete fixes at the author's
//! own target pace — extend a cue into following dead
//! air, split a dense cue at a word boundary (verbatim,
//! always), split + shift the following chain — computed
//! by the pure engine in `stykanimate_rs::
//! subtitle_timing`, whose module docs state the whole
//! model (two adjustable parameters, no hidden
//! threshold; the D-9 warning-threshold question stays
//! open and untouched). Proposals are derived state,
//! recomputed on every render and every apply press, so
//! a row can never aim at a list that has moved. Each
//! apply (one proposal, or all of them) rides the core
//! op `EditorCore::subtitle_timing_apply` (IA-W2-16-2)
//! as exactly one labelled undo step; the selection and
//! any open text session stand down after a landed fix,
//! the shift-all discipline.
//!
//! Roadmap Wave 2 (W2-99) closes RM21's named remainder
//! — the cue-text editor is genuinely multi-line. The
//! audit (findings2/w2-99.md) read the pinned Rune rev
//! first: `rune::text_input` is multi-line BY DEFAULT
//! and the kit's field builder is what forced
//! single-line, so the editor now rides the kit's
//! multi-line sibling (`design::text_area`) inside
//! `rune::auto_grow`'s bounded
//! scroll box — no stacked per-line fields, no fake
//! textarea. The keys are the engine's: plain Enter
//! still applies (the shell's committing-field rule),
//! Shift+Enter / Option+Enter break the line, and a
//! hint line under the area says so. The buffer is the
//! cue's text whole — breaks, blank lines and trailing
//! spaces ride through Apply byte-for-byte (the op's
//! one refusal, empty-after-trim, refuses identically
//! and leaves the session open); the speaker field
//! stays single-line on purpose, because the model's
//! speaker rule refuses newlines outright.
//!
//! ## The selection join (P32 IA-4)
//!
//! The panel and the timeline's subtitle lane share ONE
//! selection: `State::subtitle_selected`, an
//! `Option<usize>` into `project.subtitles` (stored
//! order everywhere — lane, panel and core ops all speak
//! the same index, so no mapping exists to get wrong).
//! Lane presses land through `subtitle_lane::press_cue`
//! (select + seek); this panel's rows press through the
//! very same handler, so a cue behaves identically
//! whichever surface selected it. Every read of the
//! selection is guarded by
//! `subtitle_lane::selection_is_live` — a selection left
//! over from a longer cue list (the project shrank under
//! it: undo, an import, a project swap) behaves as no
//! selection, the audio panel's rule for its own
//! selection, same helper name and shape.
//!
//! ## Op discipline
//!
//! Every control is wired to a real op, and the undo
//! accounting is the core's: add / update / delete /
//! style-set are `EditorCore`'s subtitle ops, exactly
//! one undo step per successful call, refusals changing
//! nothing and speaking on the status line in the ops'
//! own failure voice — the panel adds no voice of its
//! own where an op already speaks. Because an update
//! re-sorts the list, the selection FOLLOWS the cue to
//! the index the op returns; a delete clears the
//! selection outright (the index would otherwise name a
//! different cue). Steppers that would spend an undo
//! step on no change (a range edge already at its bound,
//! the scale already at its clamp) check first and
//! report instead of calling the op.
//!
//! The text edit follows the clips panel's rename
//! discipline (indices are not identity): opening the
//! editor loads the cue's text into the buffer and
//! remembers WHICH cue (index) and WHAT it carried
//! (`text_opened`); applying refuses — closing nothing,
//! changing nothing — when the cue now sitting at that
//! index no longer carries the opened text (an undo, a
//! lane edit or the live MCP server moved the world
//! under the open editor). Applying an unchanged buffer
//! closes the editor without spending an undo step.
//!
//! Import/export mirror the MCP subtitle IO tools
//! (P30) exactly, because the two surfaces must agree:
//! a parse runs BEFORE anything mutates (an unreadable
//! file or a zero-yield document refuses with the
//! reader's/parser's own error and the project is
//! untouched); a successful parse replaces the whole
//! cue list re-sorted; an import identical to the
//! current list spends NO undo step and says so; a
//! changed list spends exactly one (`snapshot()` before
//! the replace — there is deliberately no core op for
//! the bulk replace, here or in the MCP module).
//! Export is read-only: no snapshot, the success
//! sentence lands on the status line. Either way the
//! selection and any open text edit are stood down —
//! imported indices name different cues.
//!
//! ## Panel state (integrator-homed)
//!
//! `text_input`'s `on_edit` needs a `&mut TextEdit` path
//! into the shell `State` (the audio panel's audit-Q5
//! rule), so this module's state lives in `State`:
//!
//! INTEGRATOR: `crate::State` needs
//! `pub subtitle_panel: SubtitlePanelState`, initialised
//! with `SubtitlePanelState::new()` — plus the two
//! `text_focused` clauses and the project-swap clearing
//! line filed as IAs in the Q7 findings. Nothing else in
//! the shell needs to know this state exists.
//!
//! ## Labels and limits, stated
//!
//! * The section's labels resolve through the i18n
//!   tables (Wave R, R4 — the P32 IA-5 registration,
//!   landed): `subtitles` + the `subtitle_*` keys in
//!   `src/i18n.rs`, rendered via `i18n_wire::tr`, with
//!   genuine translations in the eight translated
//!   tables and English placeholders in the other 19
//!   (the tables' convention). The handlers' status
//!   sentences stay English: they are messages in the
//!   ops' voice, not labels, and the shell localises
//!   no status sentence.
//! * The composed readouts resolve the same way (Wave
//!   V, V2): the cue row's window line, the detail
//!   block's range line and cue title, the scale
//!   stepper's value and the style block's colour line
//!   are `{slot}` template keys (`subtitle_row_label`,
//!   `subtitle_range_line`, `subtitle_cue_title`,
//!   `subtitle_scale_value`, `subtitle_color_readout`
//!   — IA-V2-1), substituted at the composing site in
//!   the camera `follow_label` idiom. The words inside
//!   the range and colour lines still come from their
//!   own keys (`frames`, `subtitle_frames_unit`,
//!   `subtitle_text`, `subtitle_box`), carried through
//!   slots. What stays literal: the speaker prefix is
//!   the renderers' own composition
//!   (`subtitles::subtitle_display_lines`), the hex
//!   readout (`#RRGGBB`) and the bare stepper numbers
//!   are notation.
//! * The cue text field is the kit's multi-line
//!   area (W2-99): line breaks are edited AS breaks
//!   (Shift+Enter / Option+Enter), the buffer holds the
//!   text whole, and an unedited Apply still spends
//!   nothing (the unchanged check). Before W2-99 the
//!   kit's single-line construction edited the text as
//!   one line and its input sanitiser silently DELETED
//!   pasted line breaks, joining the lines — the audit
//!   is in findings2/w2-99.md. The one normalisation
//!   the multi-line engine itself owns: a pasted CRLF
//!   pair lands as LF, which is the sidecars' storage
//!   form anyway.
//! * Cue frames are NOT clamped to the timeline (the
//!   core ops' rule): a cue may start or end past the
//!   last frame, and the end stepper has no upper bound.
//! * The style block's colour reach (Wave V, V6 audit):
//!   the swatch rows set only the palette's eight
//!   colours, so each colour also carries RGB channel
//!   steppers (the style inspector's Phase F idiom,
//!   [`SUBTITLE_RGB_STEP`] grain) under its swatches —
//!   any `0xRRGGBB` value, including one set over MCP
//!   that marks no swatch, is reachable and adjustable
//!   here, through the same style op. The audit found
//!   NO other gap at the time: `SubtitleStyle` then had
//!   exactly four fields (position, font scale, text
//!   colour, box colour), all with full-reach controls.
//!   RM2 later added a fifth field, `font` (the burn-in
//!   renderer choice), whose panel door is the font
//!   cycler RM21 added beside the position cycler — all
//!   five fields now have panel controls. There is no
//!   background or opacity field in the model to
//!   control — `box_color` IS the backing (drawn as
//!   outline / offset stamps; no surface draws a filled
//!   box, see `subtitles.rs`) — so no such control
//!   exists, and none is faked.
//! * Drag-to-move for cues lives in the LANE (Wave R,
//!   R3 built P32's named remainder: block drags move
//!   and edge-resize cues there). This panel's range
//!   steppers, playhead buttons and nudge row are the
//!   precise numeric route to the same moves.
//!
//! ## Keys (all additive, Wave Q)
//!
//! `subtitle-section` (root), `subtitle-row-{i}` (cue
//! rows), `subtitle-add`, `subtitle-detail` (detail
//! root), `subtitle-edit-open`, `subtitle-text-field`,
//! `subtitle-speaker-field` (Wave R, IA-R21-5),
//! `subtitle-edit-apply`, `subtitle-edit-cancel`,
//! `subtitle-start-dn` / `subtitle-start-up`,
//! `subtitle-end-dn` / `subtitle-end-up`,
//! `subtitle-start-at` / `subtitle-end-at`,
//! `subtitle-delete`, `subtitle-style` (style root),
//! `subtitle-position-prev` / `subtitle-position-next`,
//! `subtitle-scale-dn` / `subtitle-scale-up`,
//! `subtitle-text-colors` / `subtitle-box-colors`
//! (swatch rows), `subtitle-text-color-{ci}` /
//! `subtitle-box-color-{ci}` (swatches 0–7),
//! `subtitle-text-{r,g,b}-row` /
//! `subtitle-box-{r,g,b}-row` (channel stepper rows,
//! Wave V, V6) with `subtitle-text-{r,g,b}-dn` /
//! `-up` and `subtitle-box-{r,g,b}-dn` / `-up`,
//! `subtitle-file` (file root), `subtitle-path` (path
//! field), `subtitle-export-srt` / `subtitle-export-vtt`,
//! `subtitle-import-srt` / `subtitle-import-vtt`.
//! Roadmap Wave 1 (RM21) adds: `subtitle-nudge-m1s` /
//! `subtitle-nudge-m100` / `subtitle-nudge-p100` /
//! `subtitle-nudge-p1s` (the detail block's nudge row),
//! `subtitle-font-prev` / `subtitle-font-next` (the
//! style block's font cycler), `subtitle-timing`
//! (timing root), `subtitle-shift-m1s` /
//! `subtitle-shift-m100` / `subtitle-shift-p100` /
//! `subtitle-shift-p1s` (shift-all), `subtitle-retime-dn`
//! / `subtitle-retime-up` / `subtitle-retime-apply`.
//! Roadmap Wave 2 (W2-16) adds: `subtitle-assist`
//! (assistant root), `subtitle-assist-target-dn` /
//! `subtitle-assist-target-up` (target-speed stepper),
//! `subtitle-assist-min-dn` / `subtitle-assist-min-up`
//! (minimum-duration stepper), `subtitle-assist-apply-{k}`
//! (per-proposal apply, k = the proposal's row),
//! `subtitle-assist-apply-all`.
//! No existing key is renamed or removed.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El, TextEdit};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::subtitle_timing::{
    timing_report, Proposal, ProposalKind, TimingParams, TimingReport, DEFAULT_MIN_SECONDS,
    DEFAULT_TARGET_CPS, MIN_SECONDS_MAX, TARGET_CPS_MAX, TARGET_CPS_MIN,
};
use stykanimate_rs::subtitles::{
    nudge_delta_frames, retimed_window, shifted_window, sort_subtitle_cues,
    subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt,
    subtitles_to_srt_counted, subtitles_to_vtt, SubtitleCue, SubtitleFont, SubtitlePosition,
    SubtitleStyle, SUBTITLE_FONT_SCALE_MAX, SUBTITLE_FONT_SCALE_MIN,
};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::icon_only_btn;
use crate::rune_sec::subtitle_lane::{press_cue, selection_is_live};
use crate::State;

/// The text a freshly added cue carries, in English.
/// [`add_cue`] resolves the `subtitle_new` i18n key,
/// whose English value is this string; the constant
/// stays as the reference the pins assert against. It
/// is a real cue the moment it lands (the core refuses
/// empty text), and adding one opens the text editor
/// on it (see [`add_cue`]), so the placeholder is one
/// keystroke away from being replaced.
pub const DEFAULT_CUE_TEXT: &str = "New subtitle";

/// The style block's colour swatches (`0xRRGGBB`): the
/// particles panel's palette shape (eight entries,
/// white and black at the ends) with a caption yellow
/// in the amber seat — the colour subtitling
/// conventionally reaches for. The panel owns its
/// palette (each panel owns its constants); colours
/// outside it (set over MCP) still show in the readout
/// line under the swatches, they simply mark no swatch.
pub const SUBTITLE_PALETTE: [u32; 8] = [
    0xFFFFFF, // white
    0xFFD94D, // caption yellow
    0xFF9F43, // orange
    0xFF5D5D, // red
    0x7BE495, // green
    0x57C7E3, // sky
    0xA184FF, // violet
    0x000000, // black
];

/// Panel-owned state: the import/export path entry and
/// the cue-text edit session. Homed in the shell
/// `State` by the integrator (see the module docs).
pub struct SubtitlePanelState {
    /// File path typed in the File block, read by the
    /// import/export buttons. Empty refuses in the
    /// buttons' own voice (there is no default file a
    /// subtitle exchange could honestly assume).
    pub path: TextEdit,
    /// The cue-text edit buffer, loaded from the target
    /// cue by [`begin_edit`].
    pub text: TextEdit,
    /// Index of the cue the text session is editing, or
    /// `None` while the editor is closed. May go stale
    /// (the world can move under an open editor);
    /// readers guard with [`edit_is_live`].
    pub text_target: Option<usize>,
    /// The text the target cue carried when the session
    /// opened (`begin_edit`). Indices are not identity:
    /// [`apply_edit`] refuses to write when the cue now
    /// at the target index does not carry this text
    /// (the clips rename panel's `opened_name`
    /// discipline).
    pub text_opened: String,
    /// The cue-speaker edit buffer (Wave R, R21 via
    /// IA-R21-5), loaded from the target cue by
    /// [`begin_edit`] — the cue's speaker name, or empty
    /// for an uncredited cue. Applying passes the buffer
    /// through the op's normalisation, so an emptied
    /// buffer clears the speaker.
    pub speaker: TextEdit,
    /// The speaker text the target cue carried when the
    /// session opened — the identity pair to
    /// `text_opened` for the speaker buffer: apply's
    /// no-change test compares BOTH buffers against
    /// their opened values.
    pub speaker_opened: String,
    /// The Timing block's retime source rate (RM21):
    /// the frame rate the cues were AUTHORED at, which
    /// [`apply_retime`] rescales to the project's
    /// current rate. `0.0` means "unset" — the steppers
    /// and the readout then speak the project's own
    /// rate, and a retime is a no-op report until the
    /// value is stepped away from it. Panel-session
    /// state only (never serialised, never an undo
    /// step to change): it is a claim about the cues'
    /// history, not project data.
    pub retime_from: f32,
    /// The Timing assistant's target reading speed
    /// (W2-16), in chars/s: the pace its proposals aim
    /// for — an author-set model parameter, NOT a
    /// warning threshold (D-9 untouched: nothing flags
    /// a cue against it; it only shapes the fixes
    /// offered). Panel-session state only, like
    /// `retime_from`: stepping it spends no undo step.
    pub timing_target_cps: f32,
    /// The Timing assistant's minimum on-screen time
    /// (W2-16), in seconds: the floor term of the
    /// engine's needed-frames formula. Panel-session
    /// state only.
    pub timing_min_seconds: f32,
}

impl SubtitlePanelState {
    pub fn new() -> Self {
        SubtitlePanelState {
            path: TextEdit::new(""),
            text: TextEdit::new(""),
            text_target: None,
            text_opened: String::new(),
            speaker: TextEdit::new(""),
            speaker_opened: String::new(),
            retime_from: 0.0,
            timing_target_cps: DEFAULT_TARGET_CPS,
            timing_min_seconds: DEFAULT_MIN_SECONDS,
        }
    }
}

impl Default for SubtitlePanelState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------
// Pure helpers (unit-tested below)
// ---------------------------------------------------------------------

/// A cue row's label: the cue's frame window in the
/// ops' 1-based voice (`start + 1` through `end`, the
/// half-open range's covered frames), then the cue's
/// text with its line breaks flattened to spaces — the
/// row is one truncated line, and the detail block
/// below carries the text whole. The window skeleton
/// is the `subtitle_row_label` template key (Wave V,
/// V2), substituted here; the cue text — user data —
/// is substituted LAST, so a cue whose text literally
/// reads `{start}` is never re-scanned for slots (the
/// camera `follow_label` rule).
pub fn cue_row_label(core: &EditorCore, cue: &SubtitleCue) -> String {
    // A credited cue's row carries its speaker as the
    // `Name: ` prefix — the same composition the
    // renderers draw (subtitles::subtitle_display_lines),
    // panel-local per IA-R21-5. The prefix is the
    // renderers' own composition, not a panel template:
    // the row mirrors the stage and does not fork it.
    let text = match &cue.speaker {
        Some(name) => format!("{name}: {}", cue.text.replace('\n', " ")),
        None => cue.text.replace('\n', " "),
    };
    i18n_wire::tr(core, "subtitle_row_label")
        .replace("{start}", &(cue.start_frame + 1).to_string())
        .replace("{end}", &cue.end_frame.to_string())
        .replace("{text}", &text)
}

/// The detail block's range line: the same window as
/// [`cue_row_label`], with the cue's length spelled
/// out. The line is the `subtitle_range_line` template
/// key (Wave V, V2); the two words ride in slots from
/// their own keys (`frames` + `subtitle_frames_unit`)
/// and the numbers are locale-invariant, so the
/// template is punctuation and slot order only.
pub fn cue_range_line(core: &EditorCore, cue: &SubtitleCue) -> String {
    i18n_wire::tr(core, "subtitle_range_line")
        .replace("{frames}", &i18n_wire::tr(core, "frames"))
        .replace("{start}", &(cue.start_frame + 1).to_string())
        .replace("{end}", &cue.end_frame.to_string())
        .replace("{count}", &(cue.end_frame - cue.start_frame).to_string())
        .replace("{unit}", &i18n_wire::tr(core, "subtitle_frames_unit"))
}

/// The live selection resolved: the selected cue's
/// index and a copy of the cue, or `None` when the
/// selection is absent or stale (the audio panel's
/// `selection_is_live` rule, through the lane's shared
/// guard).
pub fn selected_cue(core: &EditorCore, selected: Option<usize>) -> Option<(usize, SubtitleCue)> {
    if !selection_is_live(selected, core.subtitle_count()) {
        return None;
    }
    let i = selected?;
    core.subtitle_cues().get(i).map(|cue| (i, cue.clone()))
}

/// Where a new cue lands: at the playhead, one second
/// long at the project's frame rate (never shorter
/// than the model's one-frame minimum). The end is
/// NOT clamped to the timeline — the core ops' rule
/// lets a cue run past the last frame.
pub fn default_cue_window(core: &EditorCore) -> (usize, usize) {
    let len = (core.project.fps.round() as usize).max(1);
    let start = core.current;
    (start, start.saturating_add(len))
}

/// The start a start-stepper press asks for: one frame
/// in `dir` (−1/+1), bounded below by frame 0 and above
/// by the cue's last covered frame (`end - 1` — a
/// start must stay before its end). `None` when the
/// cue already sits at the bound in that direction —
/// the press then reports instead of spending an undo
/// step on no change.
pub fn stepped_start(cue: &SubtitleCue, dir: i32) -> Option<usize> {
    if dir < 0 {
        cue.start_frame.checked_sub(1)
    } else if cue.start_frame + 1 < cue.end_frame {
        Some(cue.start_frame + 1)
    } else {
        None
    }
}

/// The end an end-stepper press asks for: one frame in
/// `dir`, bounded below by `start + 1` (the one-frame
/// minimum) and unbounded above (frames are not
/// clamped to the timeline). `None` only at the lower
/// bound.
pub fn stepped_end(cue: &SubtitleCue, dir: i32) -> Option<usize> {
    if dir < 0 {
        if cue.end_frame > cue.start_frame + 1 {
            Some(cue.end_frame - 1)
        } else {
            None
        }
    } else {
        Some(cue.end_frame.saturating_add(1))
    }
}

/// A position's display label (the style stepper's
/// readout), resolved through the i18n tables
/// (`subtitle_bottom` / `subtitle_top`).
pub fn position_label(core: &EditorCore, position: SubtitlePosition) -> String {
    match position {
        SubtitlePosition::Bottom => i18n_wire::tr(core, "subtitle_bottom"),
        SubtitlePosition::Top => i18n_wire::tr(core, "subtitle_top"),
    }
}

/// The position a position-stepper press asks for:
/// the other one. The style has exactly two positions,
/// so cycling in either direction lands on the same
/// flip.
pub fn flipped_position(position: SubtitlePosition) -> SubtitlePosition {
    match position {
        SubtitlePosition::Bottom => SubtitlePosition::Top,
        SubtitlePosition::Top => SubtitlePosition::Bottom,
    }
}

/// The font scale a scale-stepper press asks for: one
/// tenth in `dir` from the SANITISED current scale
/// (the value the renderers actually use), held to
/// the style's own clamp range
/// ([`SUBTITLE_FONT_SCALE_MIN`]..=
/// [`SUBTITLE_FONT_SCALE_MAX`]). The current scale is
/// first read at its nearest tenth, so repeated
/// presses stay on the tenth grid free of float dust;
/// an off-grid stored scale (only reachable over MCP
/// or from a hand-edited file) steps a full tenth
/// from that nearest tenth.
pub fn stepped_scale(style: SubtitleStyle, dir: i32) -> f32 {
    let tenths = (style.sanitised().font_scale * 10.0).round() as i32 + dir;
    (tenths.clamp(
        (SUBTITLE_FONT_SCALE_MIN * 10.0) as i32,
        (SUBTITLE_FONT_SCALE_MAX * 10.0) as i32,
    ) as f32)
        / 10.0
}

/// The scale stepper's readout: the sanitised scale
/// at tenth precision, the stepping grain. The number
/// is formatted here and rides the `{value}` slot of
/// the `subtitle_scale_value` template (Wave V, V2 —
/// the `frame_duration_value` shape).
pub fn scale_readout(core: &EditorCore, style: SubtitleStyle) -> String {
    i18n_wire::tr(core, "subtitle_scale_value")
        .replace("{value}", &format!("{:.1}", style.sanitised().font_scale))
}

/// A stored colour as the readout's `#RRGGBB`.
pub fn hex_label(rgb: u32) -> String {
    format!("#{rgb:06X}")
}

// ---------------------------------------------------------------------
// Time domain (Roadmap Wave 1, RM21 — P-11 / A9 R9-3 + R9-2)
//
// Cue timing is frame counts at the project's frame
// rate — the same domain the SRT/WebVTT converters
// speak (`subtitles_from_srt(text, fps)`), so every
// readout below is frames ÷ fps, nothing re-derived.
// The feedback helpers MEASURE only: no cap, no wrap,
// no auto-edit (the stack cap/wrap question is the
// standing §8 decision D-9, untouched here).
// ---------------------------------------------------------------------

/// A frame's timecode at `fps`, in the SRT/WebVTT
/// shape `HH:MM:SS.mmm` — the notation the cue's own
/// sidecar files use, so the panel and the delivered
/// document read identically. Millisecond rounding is
/// to nearest. A non-positive or non-finite rate has
/// no time domain: the zero timecode comes back
/// rather than a NaN (the project rate is validated
/// at its own controls; this is a display guard).
pub fn timecode_of(frame: usize, fps: f32) -> String {
    if !(fps > 0.0) || !fps.is_finite() {
        return "00:00:00.000".to_string();
    }
    let total_ms = (frame as f64 / fps as f64 * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// A cue's duration in seconds at `fps` — the
/// half-open window's length over the rate. A
/// non-positive/non-finite rate yields 0.0 (the
/// display guard of [`timecode_of`]).
pub fn cue_duration_seconds(cue: &SubtitleCue, fps: f32) -> f32 {
    if !(fps > 0.0) || !fps.is_finite() {
        return 0.0;
    }
    (cue.end_frame - cue.start_frame) as f32 / fps
}

/// The detail block's time line: the cue's start and
/// end timecodes with its duration in seconds, at the
/// project's frame rate. The line is the
/// `subtitle_time_line` template key (IA-RM21-2);
/// the timecodes and the seconds figure are notation
/// substituted into its slots.
pub fn cue_time_line(core: &EditorCore, cue: &SubtitleCue) -> String {
    let fps = core.project.fps;
    i18n_wire::tr(core, "subtitle_time_line")
        .replace("{start}", &timecode_of(cue.start_frame, fps))
        .replace("{end}", &timecode_of(cue.end_frame, fps))
        .replace(
            "{seconds}",
            &format!("{:.1}", cue_duration_seconds(cue, fps)),
        )
}

/// A cue's reading speed: characters per second at
/// `fps`. The count is the text's characters with
/// line breaks excluded (a break is layout, not
/// reading); the duration is the half-open window's.
/// A zero duration cannot occur (the model's
/// one-frame minimum), and the rate guard of
/// [`cue_duration_seconds`] yields 0.0 rather than a
/// division by zero.
pub fn cue_cps(cue: &SubtitleCue, fps: f32) -> f32 {
    let seconds = cue_duration_seconds(cue, fps);
    if seconds <= 0.0 {
        return 0.0;
    }
    let chars = cue.text.chars().filter(|&c| c != '\n').count();
    chars as f32 / seconds
}

/// Each line's character count, in order — the
/// per-line half of the R9-2 feedback. Characters are
/// Unicode scalars (what a reader reads), never
/// bytes.
pub fn cue_line_lengths(cue: &SubtitleCue) -> Vec<usize> {
    cue.text.split('\n').map(|line| line.chars().count()).collect()
}

/// The detail block's feedback line: the cue's CPS
/// and its per-line lengths. The line is the
/// `subtitle_cps_line` template key (IA-RM21-2); the
/// lengths join with " / " (notation). Measurement
/// only — no threshold is judged here: A9 left the
/// warning threshold's default as a user decision,
/// so the panel reports the numbers and flags
/// nothing.
pub fn cue_cps_line(core: &EditorCore, cue: &SubtitleCue) -> String {
    let lengths: Vec<String> = cue_line_lengths(cue)
        .iter()
        .map(|n| n.to_string())
        .collect();
    i18n_wire::tr(core, "subtitle_cps_line")
        .replace("{cps}", &format!("{:.1}", cue_cps(cue, core.project.fps)))
        .replace("{lines}", &lengths.join(" / "))
}

/// The retime source rate in force: the panel's
/// stepped value when one is set, else the project's
/// own rate (the unset state of
/// `SubtitlePanelState::retime_from`).
pub fn effective_retime_from(panel: &SubtitlePanelState, project_fps: f32) -> f32 {
    if panel.retime_from > 0.0 {
        panel.retime_from
    } else {
        project_fps
    }
}

/// The Timing assistant's parameters in force (W2-16):
/// the panel's stepped values held to the model's
/// ranges (the engine's `sanitised` backstop — the
/// steppers already step inside them).
pub fn timing_params(panel: &SubtitlePanelState) -> TimingParams {
    TimingParams {
        target_cps: panel.timing_target_cps,
        min_seconds: panel.timing_min_seconds,
    }
    .sanitised()
}

/// The Timing assistant's current report over the
/// project's cues (W2-16), recomputed from the live
/// list on every call: proposals are derived state,
/// never stored, so a rendered row and the press that
/// follows it always speak about the same list (the
/// handlers-here resolve their targets at press time,
/// the panel's standing discipline).
pub fn timing_report_for(core: &EditorCore, panel: &SubtitlePanelState) -> TimingReport {
    timing_report(core.subtitle_cues(), core.project.fps, &timing_params(panel))
}

/// One proposal's line (W2-16): the cue it addresses
/// (1-based, the ops' voice), the fix it makes, and the
/// measured pace before → after. The skeletons are the
/// `subtitle_assist_extend` / `subtitle_assist_split` /
/// `subtitle_assist_split_shift` template keys
/// (IA-W2-16-3). Every substituted value is a number or
/// the localised word for a cue — cue TEXT is never
/// composed into the line, so no slot can be re-scanned
/// by user data (the `cue_row_label` rule, trivially
/// satisfied here).
pub fn assist_proposal_line(core: &EditorCore, proposal: &Proposal) -> String {
    let cue_word = i18n_wire::tr(core, "subtitle_cue");
    let index = (proposal.cue_index + 1).to_string();
    let cps = format!("{:.1}", proposal.cps_before);
    let after = format!("{:.1}", proposal.cps_after);
    match proposal.kind {
        ProposalKind::Extend => {
            let grown = &proposal.replacement[0];
            let seconds =
                (grown.end_frame - grown.start_frame) as f32 / core.project.fps;
            i18n_wire::tr(core, "subtitle_assist_extend")
                .replace("{cue}", &cue_word)
                .replace("{index}", &index)
                .replace("{seconds}", &format!("{seconds:.1}"))
                .replace("{cps}", &cps)
                .replace("{after}", &after)
        }
        ProposalKind::Split => {
            if proposal.shift_following_by > 0 {
                i18n_wire::tr(core, "subtitle_assist_split_shift")
                    .replace("{cue}", &cue_word)
                    .replace("{index}", &index)
                    .replace("{shift}", &proposal.shift_following_by.to_string())
                    .replace("{cps}", &cps)
                    .replace("{after}", &after)
            } else {
                i18n_wire::tr(core, "subtitle_assist_split")
                    .replace("{cue}", &cue_word)
                    .replace("{index}", &index)
                    .replace("{cps}", &cps)
                    .replace("{after}", &after)
            }
        }
    }
}

/// A font's display label (the style cycler's
/// readout), resolved through the i18n tables
/// (`subtitle_font_shaped` / `subtitle_font_pixel` —
/// IA-RM21-2).
pub fn font_label(core: &EditorCore, font: SubtitleFont) -> String {
    match font {
        SubtitleFont::Shaped => i18n_wire::tr(core, "subtitle_font_shaped"),
        SubtitleFont::Pixel => i18n_wire::tr(core, "subtitle_font_pixel"),
    }
}

/// The font a font-cycler press asks for: the other
/// one. The style has exactly two fonts (RM2), so
/// cycling in either direction lands on the same
/// flip — the position cycler's shape.
pub fn flipped_font(font: SubtitleFont) -> SubtitleFont {
    match font {
        SubtitleFont::Shaped => SubtitleFont::Pixel,
        SubtitleFont::Pixel => SubtitleFont::Shaped,
    }
}

/// Per-press channel step for the style block's RGB
/// channel steppers (Wave V, V6): the style
/// inspector's grain (`inspector_style::RGB_STEP`),
/// named locally because each panel owns its
/// constants — the VALUE matches, so every colour
/// stepper in the shell moves alike.
pub const SUBTITLE_RGB_STEP: i32 = 8;

/// Channel `channel` (0 = R, 1 = G, 2 = B) of a
/// `0xRRGGBB` colour. An out-of-range channel reads
/// 0 — the steppers only ever pass 0–2, and
/// [`stepped_channel`] refuses out-of-range channels
/// outright.
pub fn channel_value(rgb: u32, channel: usize) -> u8 {
    match channel {
        0 => ((rgb >> 16) & 0xFF) as u8,
        1 => ((rgb >> 8) & 0xFF) as u8,
        2 => (rgb & 0xFF) as u8,
        _ => 0,
    }
}

/// A `0xRRGGBB` colour with channel `channel` stepped
/// one press in `dir` (−1 / +1): the channel moves by
/// [`SUBTITLE_RGB_STEP`], clamped to 0..=255, the
/// other channels untouched. An out-of-range channel
/// index returns the colour unchanged, as does a step
/// at the clamp bound — the step handlers treat
/// "unchanged" as no change and spend no undo step.
pub fn stepped_channel(rgb: u32, channel: usize, dir: i32) -> u32 {
    if channel > 2 {
        return rgb;
    }
    let shift = 16 - 8 * channel as u32;
    let stepped =
        (channel_value(rgb, channel) as i32 + dir * SUBTITLE_RGB_STEP).clamp(0, 255) as u32;
    (rgb & !(0xFF << shift)) | (stepped << shift)
}

/// The pure-channel preview colour for a channel
/// row's mini-swatch: the channel's own value in its
/// own slot, the style inspector's idiom.
pub fn channel_preview(value: u8, channel: usize) -> u32 {
    let shift = 16 - 8 * (channel.min(2)) as u32;
    (value as u32) << shift
}

/// Whether the text session still names the cue it
/// opened: the target IS the shell's live selection,
/// the index is in range, and the cue there still
/// carries the opened text. [`apply_edit`]'s write
/// guard and the detail block's render guard — one
/// rule, both sides. (The selection lives on the
/// shell `State`, not in the core — the core has no
/// subtitle selection, per P32's contract — so this
/// guard takes the whole state.)
pub fn edit_is_live(s: &State) -> bool {
    let panel = &s.subtitle_panel;
    let Some(target) = panel.text_target else {
        return false;
    };
    if s.subtitle_selected != Some(target) {
        return false;
    }
    match s.core.subtitle_cues().get(target) {
        Some(cue) => cue.text == panel.text_opened,
        None => false,
    }
}

// ---------------------------------------------------------------------
// Handlers (factored out of the closures so they are
// unit-testable without a runtime gesture — the
// timeline's press-handler discipline). Every handler
// resolves the live selection itself, so a stale view
// can never aim an op at the wrong cue.
// ---------------------------------------------------------------------

/// Select cue `i` and seek the playhead to its first
/// frame — the lane's own press handler, shared
/// verbatim so both surfaces behave identically. No
/// undo step (selection and playhead are runtime
/// state).
pub fn select_cue(s: &mut State, i: usize) {
    press_cue(s, i);
}

/// Add a cue at the playhead (see
/// [`default_cue_window`]) carrying the `subtitle_new`
/// label ([`DEFAULT_CUE_TEXT`] in English), select it
/// at the index the op returns, and open the text
/// editor on it so the placeholder is immediately
/// replaceable. One undo step (the op's). A refusal
/// is the op's own status; nothing else moves.
pub fn add_cue(s: &mut State) {
    let (start, end) = default_cue_window(&s.core);
    let text = i18n_wire::tr(&s.core, "subtitle_new");
    if let Some(i) = s.core.subtitle_add(start, end, &text, None) {
        s.subtitle_selected = Some(i);
        begin_edit(s);
    }
}

/// Open the text editor on the live selection: load
/// the cue's text into the buffer and remember the
/// cue's index and text (the identity the apply guard
/// checks). With no live selection there is nothing
/// to open — the detail block only renders this
/// button's row under a live selection, so the guard
/// is a stale-view backstop, not a path.
pub fn begin_edit(s: &mut State) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    s.subtitle_panel.text = TextEdit::new(cue.text.clone());
    s.subtitle_panel.text_target = Some(i);
    s.subtitle_panel.text_opened = cue.text;
    s.subtitle_panel.speaker = TextEdit::new(cue.speaker.clone().unwrap_or_default());
    s.subtitle_panel.speaker_opened = cue.speaker.unwrap_or_default();
}

/// Stand the text session down: forget the target and
/// replace the buffer with a fresh one — a replaced
/// buffer cannot keep a stale `focused` flag alive
/// after the field unmounts (the Home rename wart P9
/// documented, designed out here rather than routed
/// around).
pub fn close_edit(s: &mut State) {
    s.subtitle_panel.text_target = None;
    s.subtitle_panel.text_opened = String::new();
    s.subtitle_panel.text = TextEdit::new("");
    s.subtitle_panel.speaker_opened = String::new();
    s.subtitle_panel.speaker = TextEdit::new("");
}

/// Apply the text session: write the buffer back over
/// the target cue's own range through
/// `EditorCore::subtitle_update` (one undo step), the
/// selection following the cue to its post-sort index,
/// and close the session. The guards, in order:
///
/// * the session no longer names its cue
///   ([`edit_is_live`] fails — the world moved): close
///   the session, change nothing, and say so;
/// * both buffers are unchanged (text AND speaker —
///   IA-R21-5's compare-both rule): close the session
///   and say so, spending no undo step;
/// * the op refuses (empty text): the op's status
///   stands and the session stays OPEN, so the text
///   can be fixed in place.
pub fn apply_edit(s: &mut State) {
    if !edit_is_live(s) {
        let stale = s.subtitle_panel.text_target.is_some();
        close_edit(s);
        if stale {
            s.core.status =
                "Subtitle text edit failed: the cue changed — reopen the editor.".to_string();
        }
        return;
    }
    let i = s.subtitle_panel.text_target.unwrap_or_default();
    let text = s.subtitle_panel.text.text.clone();
    let speaker = s.subtitle_panel.speaker.text.clone();
    if text == s.subtitle_panel.text_opened && speaker == s.subtitle_panel.speaker_opened {
        close_edit(s);
        s.core.status = format!("Subtitle {} text unchanged.", i + 1);
        return;
    }
    let Some((_, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    if let Some(new_i) =
        s.core.subtitle_update(i, cue.start_frame, cue.end_frame, &text, Some(speaker.as_str()))
    {
        s.subtitle_selected = Some(new_i);
        close_edit(s);
    }
}

/// Step the selected cue's start one frame in `dir`
/// through `EditorCore::subtitle_update` (one undo
/// step), the selection following the cue. At the
/// bound the press reports instead of spending a step
/// on no change.
pub fn step_start(s: &mut State, dir: i32) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    match stepped_start(&cue, dir) {
        Some(start) => {
            if let Some(new_i) = s.core.subtitle_update(i, start, cue.end_frame, &cue.text, cue.speaker.as_deref()) {
                s.subtitle_selected = Some(new_i);
            }
        }
        None if dir < 0 => {
            s.core.status = format!("Subtitle {} starts at the first frame.", i + 1);
        }
        None => {
            s.core.status = format!("Subtitle {} start already meets its end.", i + 1);
        }
    }
}

/// Step the selected cue's end one frame in `dir`;
/// the end half of [`step_start`]'s discipline. The
/// end has no upper bound, so only the one-frame
/// minimum reports.
pub fn step_end(s: &mut State, dir: i32) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    match stepped_end(&cue, dir) {
        Some(end) => {
            if let Some(new_i) = s.core.subtitle_update(i, cue.start_frame, end, &cue.text, cue.speaker.as_deref()) {
                s.subtitle_selected = Some(new_i);
            }
        }
        None => {
            s.core.status = format!("Subtitle {} is one frame long.", i + 1);
        }
    }
}

/// Move the selected cue's start to the playhead
/// frame (one undo step through the update op). The
/// playhead must sit before the cue's end — a start
/// at or past it would invert the cue — and a press
/// with the start already there reports instead of
/// spending a step.
pub fn start_at_playhead(s: &mut State) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    let at = s.core.current;
    if at == cue.start_frame {
        s.core.status = format!("Subtitle {} already starts at the playhead.", i + 1);
        return;
    }
    if at >= cue.end_frame {
        s.core.status = format!(
            "Subtitle start move failed: the playhead is not before subtitle {}'s end.",
            i + 1
        );
        return;
    }
    if let Some(new_i) = s.core.subtitle_update(i, at, cue.end_frame, &cue.text, cue.speaker.as_deref()) {
        s.subtitle_selected = Some(new_i);
    }
}

/// Move the selected cue's end to just past the
/// playhead frame, so the playhead becomes the cue's
/// last covered frame (one undo step). The playhead
/// must sit at or after the cue's start, and a press
/// with the end already there reports instead.
pub fn end_at_playhead(s: &mut State) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    let at = s.core.current;
    if at + 1 == cue.end_frame {
        s.core.status = format!("Subtitle {} already ends at the playhead.", i + 1);
        return;
    }
    if at < cue.start_frame {
        s.core.status = format!(
            "Subtitle end move failed: the playhead is not at or after subtitle {}'s start.",
            i + 1
        );
        return;
    }
    if let Some(new_i) = s.core.subtitle_update(i, cue.start_frame, at + 1, &cue.text, cue.speaker.as_deref()) {
        s.subtitle_selected = Some(new_i);
    }
}

/// Nudge the selected cue by `ms` milliseconds
/// (RM21 — the time-domain half of the move controls;
/// the frame steppers stay). The WHOLE window moves
/// through `EditorCore::subtitle_update` (one undo
/// step, duration preserved — see [`shifted_window`]),
/// the selection following the cue. A nudge that
/// cannot move (the cue is against frame 0, moving
/// back) reports instead of spending a step.
pub fn nudge_selected(s: &mut State, ms: i32) {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    let delta = nudge_delta_frames(ms, s.core.project.fps);
    match shifted_window(&cue, delta) {
        Some((start, end)) => {
            if let Some(new_i) =
                s.core.subtitle_update(i, start, end, &cue.text, cue.speaker.as_deref())
            {
                s.subtitle_selected = Some(new_i);
            }
        }
        None => {
            s.core.status = format!("Subtitle {} already starts at the first frame.", i + 1);
        }
    }
}

/// Shift EVERY cue by `ms` milliseconds (RM21) — the
/// bulk half of the time-domain controls. The bulk
/// discipline itself (clone, shift, re-sort, one
/// snapshot) is the core op's —
/// `EditorCore::subtitle_shift_all` (IA-RM21-3), which
/// the MCP `subtitle_shift_all` tool also calls, so
/// the panel and the tool cannot drift. This shell
/// wrapper adds the two things the core never sees:
/// the selection and any open text session stand down
/// after a shift that moved something (indices may
/// name different cues after clamping collisions
/// re-sort).
pub fn shift_all(s: &mut State, ms: i32) {
    if s.core.subtitle_shift_all(ms) > 0 {
        s.subtitle_selected = None;
        close_edit(s);
    }
}

/// Step the Timing block's retime source rate by one
/// frame-per-second in `dir`, held to 1..=120. Panel
/// state only — no undo step, no status: the readout
/// beside the stepper is the feedback. Stepping from
/// the unset state starts from the project's rate
/// ([`effective_retime_from`]).
pub fn step_retime_from(s: &mut State, dir: i32) {
    let base = effective_retime_from(&s.subtitle_panel, s.core.project.fps);
    let next = (base + dir as f32).clamp(1.0, 120.0);
    s.subtitle_panel.retime_from = next;
}

/// Retime every cue from the Timing block's source
/// rate to the project's current rate (RM21 — fps
/// retiming made explicit, never silent: the press is
/// the offer A9 R9-3 asks for, and the status line
/// names BOTH rates). One undo step on the import's
/// bulk discipline ([`shift_all`]'s note; the core op is
/// `EditorCore::subtitle_retime`, IA-RM21-3). A source rate equal
/// to the project rate — or a rescale that lands every
/// cue where it already sits — reports and spends
/// nothing.
pub fn apply_retime(s: &mut State) {
    let from = effective_retime_from(&s.subtitle_panel, s.core.project.fps);
    if s.core.subtitle_retime(from) > 0 {
        s.subtitle_selected = None;
        close_edit(s);
    }
}

/// Step the Timing assistant's target speed by one
/// char/s in `dir` (W2-16), held to the model's range
/// ([`TARGET_CPS_MIN`]..=[`TARGET_CPS_MAX`]). Panel
/// state only — no undo step, no status: the readout
/// and the recomputed proposal rows are the feedback.
pub fn step_timing_target(s: &mut State, dir: i32) {
    let next = (s.subtitle_panel.timing_target_cps + dir as f32)
        .clamp(TARGET_CPS_MIN, TARGET_CPS_MAX);
    s.subtitle_panel.timing_target_cps = next;
}

/// Step the Timing assistant's minimum on-screen time
/// by a quarter second in `dir` (W2-16), held to
/// 0..=[`MIN_SECONDS_MAX`]. Panel state only, like
/// [`step_timing_target`].
pub fn step_timing_min(s: &mut State, dir: i32) {
    let next = (s.subtitle_panel.timing_min_seconds + 0.25 * dir as f32)
        .clamp(0.0, MIN_SECONDS_MAX);
    s.subtitle_panel.timing_min_seconds = next;
}

/// Apply ONE Timing assistant proposal (W2-16): `k`
/// indexes the report recomputed at press time by the
/// core op (`EditorCore::subtitle_timing_apply`,
/// IA-W2-16-2), so the fix applied is the fix the row
/// showed. Exactly one undo step when a fix lands; the
/// selection and any open text session stand down after
/// it (a landed split renumbers the list), the
/// shift-all discipline. A press against a report that
/// no longer holds `k` spends nothing — the op reports
/// and the world stays.
pub fn apply_timing_proposal(s: &mut State, k: usize) {
    if s.core
        .subtitle_timing_apply(&timing_params(&s.subtitle_panel), Some(k))
        > 0
    {
        s.subtitle_selected = None;
        close_edit(s);
    }
}

/// Apply EVERY Timing assistant proposal (W2-16) as
/// one undo step — the batch twin of
/// [`apply_timing_proposal`], same stand-down, same
/// no-change honesty (an all-comfortable list reports
/// and spends nothing).
pub fn apply_all_timing_proposals(s: &mut State) {
    if s.core
        .subtitle_timing_apply(&timing_params(&s.subtitle_panel), None)
        > 0
    {
        s.subtitle_selected = None;
        close_edit(s);
    }
}

/// Delete the selected cue through
/// `EditorCore::subtitle_delete` (one undo step) and
/// clear the selection: the deleted cue's index would
/// otherwise name the cue that slid into its place.
/// Any open text session stands down with it.
pub fn delete_selected(s: &mut State) {
    let Some((i, _)) = selected_cue(&s.core, s.subtitle_selected) else {
        return;
    };
    if s.core.subtitle_delete(i) {
        s.subtitle_selected = None;
        close_edit(s);
    }
}

/// Flip the subtitle style's position (Bottom ↔ Top)
/// through `EditorCore::set_subtitle_style` — one
/// undo step, the op's own status either way.
pub fn cycle_position(s: &mut State) {
    let mut style = s.core.subtitle_style();
    style.position = flipped_position(style.position);
    let _ = s.core.set_subtitle_style(style);
}

/// Flip the subtitle style's font (Shaped ↔ Pixel)
/// through `EditorCore::set_subtitle_style` — one
/// undo step, the op's own status either way. This is
/// IA-RM2-3's panel door: RM2 routed the burn-in by
/// `style.font` and gave the choice to MCP and the
/// project file; the cycler gives it to the panel.
pub fn cycle_font(s: &mut State) {
    let mut style = s.core.subtitle_style();
    style.font = flipped_font(style.font);
    let _ = s.core.set_subtitle_style(style);
}

/// Step the subtitle style's font scale one tenth in
/// `dir` (one undo step through the style op). At the
/// clamp bound the press reports instead of spending
/// a step on no change.
pub fn step_scale(s: &mut State, dir: i32) {
    let style = s.core.subtitle_style();
    let next = stepped_scale(style, dir);
    if (next - style.sanitised().font_scale).abs() < f32::EPSILON {
        s.core.status = if dir < 0 {
            "Subtitle font scale is at its smallest.".to_string()
        } else {
            "Subtitle font scale is at its largest.".to_string()
        };
        return;
    }
    let mut style = style;
    style.font_scale = next;
    let _ = s.core.set_subtitle_style(style);
}

/// Set the subtitle style's text colour (a swatch
/// press; one undo step through the style op).
/// Pressing the colour already in force does nothing
/// — the swatch already shows it selected, and a step
/// spent on no change would be a lie in the undo
/// history.
pub fn set_text_color(s: &mut State, rgb: u32) {
    let mut style = s.core.subtitle_style();
    if style.text_color == rgb {
        return;
    }
    style.text_color = rgb;
    let _ = s.core.set_subtitle_style(style);
}

/// Set the subtitle style's box colour; the box half
/// of [`set_text_color`]'s discipline.
pub fn set_box_color(s: &mut State, rgb: u32) {
    let mut style = s.core.subtitle_style();
    if style.box_color == rgb {
        return;
    }
    style.box_color = rgb;
    let _ = s.core.set_subtitle_style(style);
}

/// Step the subtitle style's text colour one channel
/// press (channel 0 = R, 1 = G, 2 = B — see
/// [`stepped_channel`]) through
/// `EditorCore::set_subtitle_style`: one undo step,
/// the op's own status. A press that changes nothing
/// (the channel already at its clamp bound, a bad
/// channel index) spends no step — the row renders
/// that direction inert, and this guard is the
/// backstop. Together with the swatches this gives
/// the colour its full reach: the swatches land the
/// palette, the steppers walk anywhere from there,
/// including onto colours no swatch carries.
pub fn step_text_channel(s: &mut State, channel: usize, dir: i32) {
    let mut style = s.core.subtitle_style();
    let next = stepped_channel(style.text_color, channel, dir);
    if next == style.text_color {
        return;
    }
    style.text_color = next;
    let _ = s.core.set_subtitle_style(style);
}

/// Step the subtitle style's box colour one channel
/// press; the box half of [`step_text_channel`]'s
/// discipline.
pub fn step_box_channel(s: &mut State, channel: usize, dir: i32) {
    let mut style = s.core.subtitle_style();
    let next = stepped_channel(style.box_color, channel, dir);
    if next == style.box_color {
        return;
    }
    style.box_color = next;
    let _ = s.core.set_subtitle_style(style);
}

/// Which subtitle file format a File-block button
/// speaks: the SRT and WebVTT converters are separate
/// functions with separate refusal disciplines (the
/// WEBVTT header rule is VTT-only), so the format is
/// never guessed from the path's extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubtitleFileFormat {
    Srt,
    Vtt,
}

impl SubtitleFileFormat {
    /// The format's name as the status lines use it
    /// (the MCP IO module's labels: "SRT" / "WebVTT").
    pub fn label(self) -> &'static str {
        match self {
            SubtitleFileFormat::Srt => "SRT",
            SubtitleFileFormat::Vtt => "WebVTT",
        }
    }

    fn to_document(self, project: &stykanimate_rs::model::Project) -> String {
        match self {
            SubtitleFileFormat::Srt => subtitles_to_srt(project),
            SubtitleFileFormat::Vtt => subtitles_to_vtt(project),
        }
    }

    fn from_document(
        self,
        text: &str,
        fps: f32,
    ) -> Result<Vec<SubtitleCue>, String> {
        match self {
            SubtitleFileFormat::Srt => subtitles_from_srt(text, fps),
            SubtitleFileFormat::Vtt => subtitles_from_vtt(text, fps),
        }
    }
}

/// The File block's path, trimmed. An empty field is
/// the buttons' one local refusal — there is no
/// default subtitle file to assume.
fn file_path(s: &State) -> Option<String> {
    let path = s.subtitle_panel.path.text.trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Export the project's cues to the File block's path
/// in `format` (the MCP export's discipline: read-only
/// — no undo step; the parent directory is created
/// the way `mcp_subtitle_io` creates it). Success and
/// refusal both land on the status line, in the
/// panel's voice; an SRT success that dropped speaker
/// credits says so on the same line (Wave S, S9).
pub fn export_file(s: &mut State, format: SubtitleFileFormat) {
    let Some(path) = file_path(s) else {
        s.core.status = "Subtitle export failed: no file path.".to_string();
        return;
    };
    let path_ref = std::path::Path::new(&path);
    if let Some(parent) = path_ref.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                s.core.status = format!(
                    "Subtitle export failed: could not create directory {}: {e}",
                    parent.display()
                );
                return;
            }
        }
    }
    // An SRT export goes through the counted converter
    // (Wave S, S9): SRT cannot write cue speakers, and
    // the success line below says so when it dropped
    // any. WebVTT keeps speakers, so its document comes
    // from `to_document` with nothing to report.
    let (document, speakers_dropped) = match format {
        SubtitleFileFormat::Srt => {
            let export = subtitles_to_srt_counted(&s.core.project);
            (export.text, export.speakers_dropped)
        }
        SubtitleFileFormat::Vtt => (format.to_document(&s.core.project), 0),
    };
    match std::fs::write(path_ref, &document) {
        Ok(()) => {
            let count = s.core.subtitle_count();
            s.core.status = if speakers_dropped > 0 {
                format!(
                    "Exported {count} subtitle cue(s) to {path} — {speakers_dropped} speaker credit(s) not written (SRT has no speaker syntax)."
                )
            } else {
                format!("Exported {count} subtitle cue(s) to {path}.")
            };
        }
        Err(e) => {
            s.core.status = format!("Subtitle export failed: could not write {path}: {e}");
        }
    }
}

/// Import cues from the File block's path in `format`,
/// replacing the whole cue list (the MCP import's
/// discipline, mirrored exactly): read and parse
/// BEFORE anything mutates; the parsed list is
/// re-sorted; an import identical to the current list
/// spends no undo step and says so; a changed list
/// spends exactly one (`snapshot()` before the
/// replace — deliberately no core op exists for the
/// bulk replace). A successful import stands the
/// selection and any open text session down: the
/// imported list's indices name different cues.
pub fn import_file(s: &mut State, format: SubtitleFileFormat) {
    let label = format.label();
    let Some(path) = file_path(s) else {
        s.core.status = "Subtitle import failed: no file path.".to_string();
        return;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            s.core.status = format!("Subtitle import failed: could not read {path}: {e}");
            return;
        }
    };
    let mut cues = match format.from_document(&text, s.core.project.fps) {
        Ok(cues) => cues,
        Err(e) => {
            s.core.status = format!("Subtitle import failed: {e}");
            return;
        }
    };
    sort_subtitle_cues(&mut cues);
    if cues == s.core.project.subtitles {
        s.core.status = format!("{label} import matched the current subtitles; nothing changed.");
        return;
    }
    // Speaker asymmetry (A9 G9c, closed by RM21): an
    // SRT import replacing a speaker-carrying list
    // drops credits the format cannot carry — the
    // export side counts this loss (Wave S, S9), so
    // the import side now names it too, in the same
    // sentence shape. WebVTT keeps speakers, so its
    // imports never grow the clause.
    let credited_before = s
        .core
        .project
        .subtitles
        .iter()
        .filter(|c| c.speaker.is_some())
        .count();
    let credited_after = cues.iter().filter(|c| c.speaker.is_some()).count();
    let dropped = credited_before.saturating_sub(credited_after);
    let count = cues.len();
    s.core.snapshot_labeled("Import subtitles");
    s.core.project.subtitles = cues;
    s.subtitle_selected = None;
    close_edit(s);
    s.core.status = if format == SubtitleFileFormat::Srt && dropped > 0 {
        format!(
            "Imported {count} subtitle(s) from {label} — {dropped} speaker credit(s) dropped (SRT has no speaker syntax)."
        )
    } else {
        format!("Imported {count} subtitle(s) from {label}.")
    };
}

// ---------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------

/// The Subtitles section: cue list, add, the selected
/// cue's detail editor, the style block and the file
/// block. Mounted at the foot of the Audio workspace's
/// panel by `audio_panel` (the section call there is
/// the whole mount). The section renders in full for a
/// cue-less project too — Add and Import are how cues
/// arrive, and the style is project-level — with the
/// kit empty state standing where the list would be.
pub fn subtitle_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let cues = core.subtitle_cues();
    let mut section = col()
        .key("subtitle-section")
        .w_full()
        .gap(8.0)
        .child(design::section_label(&i18n_wire::tr(core, "subtitles"), t));
    if cues.is_empty() {
        // W2-57 (PL-6 completion, IA-RM28-6): the state
        // offers its filling action — Add cue, the
        // section's own add, the same handler the Add
        // button below runs.
        section = section.child(design::empty_state_action(
            "star",
            &i18n_wire::tr(core, "subtitle_none"),
            "subtitle-empty-add",
            &i18n_wire::tr(core, "subtitle_add"),
            t,
            |s| add_cue(s),
        ));
    } else {
        for (i, cue) in cues.iter().enumerate() {
            let selected = s.subtitle_selected == Some(i);
            section = section.child(
                design::list_row(&format!("subtitle-row-{i}"), selected, t)
                    .w_full()
                    .on_press(move |s: &mut State| select_cue(s, i))
                    .child(
                        text(cue_row_label(core, cue))
                            .font(type_::BODY)
                            .color(t.text)
                            .grow(1.0)
                            .truncate(),
                    ),
            );
        }
    }
    section = section.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn(
                "subtitle-add",
                &i18n_wire::tr(core, "subtitle_add"),
                false,
                t,
                |s| add_cue(s),
            )),
    );
    if !cues.is_empty() {
        section = section.child(detail_block(s, t));
        section = section.child(timing_block(s, t));
        section = section.child(assist_block(s, t));
    }
    section = section.child(style_block(s, t));
    section = section.child(file_block(s, t));
    section
}

/// The Timing block (RM21): the whole-list time
/// controls — shift-all by time grains and the
/// explicit fps retime. Mounted only when cues exist
/// (both controls are meaningless over an empty list,
/// and their handlers refuse silently there as a
/// backstop). The bulk discipline is [`shift_all`]'s:
/// one undo step per applied press, no-change reports
/// instead of steps.
fn timing_block(s: &State, t: &Theme) -> El {
    let from = effective_retime_from(&s.subtitle_panel, s.core.project.fps);
    let from_label = if from.fract() == 0.0 {
        format!("{from:.0}")
    } else {
        format!("{from:.2}")
    };
    col()
        .key("subtitle-timing")
        .w_full()
        .gap(8.0)
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "subtitle_timing"),
            t,
        ))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_shift_all"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-shift-m1s", "-1s", false, t, |s| {
                    shift_all(s, -1000)
                }))
                .child(btn("subtitle-shift-m100", "-0.1s", false, t, |s| {
                    shift_all(s, -100)
                }))
                .child(btn("subtitle-shift-p100", "+0.1s", false, t, |s| {
                    shift_all(s, 100)
                }))
                .child(btn("subtitle-shift-p1s", "+1s", false, t, |s| {
                    shift_all(s, 1000)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_retime"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-retime-dn", "−", false, t, |s| {
                    step_retime_from(s, -1)
                }))
                .child(design::stepper_value(&from_label, t))
                .child(btn("subtitle-retime-up", "+", false, t, |s| {
                    step_retime_from(s, 1)
                }))
                .child(btn(
                    "subtitle-retime-apply",
                    &i18n_wire::tr(&s.core, "subtitle_retime_apply"),
                    false,
                    t,
                    |s| apply_retime(s),
                )),
        )
}

/// How many proposal rows the assistant block gives
/// their own Apply buttons (W2-16): the first fixes in
/// cue order. The count line and Apply-all cover the
/// rest, so a long report cannot grow the card without
/// bound — every proposal stays reachable, the first
/// few stay one press away.
const ASSIST_MAX_ROWS: usize = 6;

/// The Timing assistant block (W2-16): under the Timing
/// block, mounted when cues exist. RM21 measured every
/// cue; this block proposes concrete fixes at the
/// author's own target pace — the model's two
/// parameters as steppers, one row per proposed fix
/// with its own Apply, an Apply-all with the count, and
/// the no-room count when a cue cannot be helped. The
/// report is recomputed on every render from the live
/// list, so a row always names the fix its press would
/// apply. When nothing needs fixing the block says so
/// in the model's terms (every cue fits the TARGET —
/// the author's pace, not a judged threshold; D-9
/// untouched).
fn assist_block(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let report = timing_report_for(core, &s.subtitle_panel);
    let params = timing_params(&s.subtitle_panel);
    let mut block = col()
        .key("subtitle-assist")
        .w_full()
        .gap(8.0)
        .child(design::section_label(
            &i18n_wire::tr(core, "subtitle_assist"),
            t,
        ))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    design::section_label(
                        &i18n_wire::tr(core, "subtitle_assist_target"),
                        t,
                    )
                    .grow(1.0),
                )
                .child(btn("subtitle-assist-target-dn", "−", false, t, |s| {
                    step_timing_target(s, -1)
                }))
                .child(design::stepper_value(&format!("{:.0}", params.target_cps), t))
                .child(btn("subtitle-assist-target-up", "+", false, t, |s| {
                    step_timing_target(s, 1)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    design::section_label(
                        &i18n_wire::tr(core, "subtitle_assist_min"),
                        t,
                    )
                    .grow(1.0),
                )
                .child(btn("subtitle-assist-min-dn", "−", false, t, |s| {
                    step_timing_min(s, -1)
                }))
                .child(design::stepper_value(
                    &format!("{:.2}", params.min_seconds),
                    t,
                ))
                .child(btn("subtitle-assist-min-up", "+", false, t, |s| {
                    step_timing_min(s, 1)
                })),
        );
    if report.proposals.is_empty() && report.stuck == 0 {
        return block.child(
            text(i18n_wire::tr(core, "subtitle_assist_none"))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for (k, proposal) in report.proposals.iter().take(ASSIST_MAX_ROWS).enumerate() {
        block = block.child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    text(assist_proposal_line(core, proposal))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0)
                        .truncate(),
                )
                .child(btn(
                    &format!("subtitle-assist-apply-{k}"),
                    &i18n_wire::tr(core, "subtitle_assist_apply"),
                    false,
                    t,
                    move |s| apply_timing_proposal(s, k),
                )),
        );
    }
    if !report.proposals.is_empty() {
        block = block.child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "subtitle-assist-apply-all",
                    &i18n_wire::tr(core, "subtitle_assist_apply_all"),
                    false,
                    t,
                    |s| apply_all_timing_proposals(s),
                ))
                .child(
                    text(
                        i18n_wire::tr(core, "subtitle_assist_count")
                            .replace("{count}", &report.proposals.len().to_string()),
                    )
                    .font(type_::LABEL)
                    .color(t.text_secondary),
                ),
        );
    }
    if report.stuck > 0 {
        block = block.child(
            text(
                i18n_wire::tr(core, "subtitle_assist_stuck")
                    .replace("{count}", &report.stuck.to_string()),
            )
            .font(type_::LABEL)
            .color(t.text_secondary),
        );
    }
    block
}

/// The cue-text area's grow bounds (W2-99): the
/// multi-line editor opens at least two lines tall
/// (a one-line box would hide that breaks are
/// editable at all) and grows with the text to six,
/// then scrolls inside `rune::auto_grow`'s viewport
/// with the caret kept in view — a long cue can
/// never push the detail card open without bound.
const TEXT_AREA_MIN_LINES: usize = 2;
/// See [`TEXT_AREA_MIN_LINES`].
const TEXT_AREA_MAX_LINES: usize = 6;

/// The selected cue's editor: title and range readout,
/// the cue's text (or the open text session), the
/// start/end steppers, the playhead placement row and
/// delete. With cues present but no live selection it
/// renders the kit guidance state (the particles
/// detail block's pattern) instead of dead controls.
fn detail_block(s: &State, t: &Theme) -> El {
    let Some((i, cue)) = selected_cue(&s.core, s.subtitle_selected) else {
        return col()
            .key("subtitle-detail")
            .w_full()
            .child(design::empty_state(
                "star",
                &i18n_wire::tr(&s.core, "subtitle_select"),
                t,
            ));
    };
    let mut block = col()
        .key("subtitle-detail")
        .w_full()
        .gap(8.0)
        .child(
            text(
                i18n_wire::tr(&s.core, "subtitle_cue_title")
                    .replace("{cue}", &i18n_wire::tr(&s.core, "subtitle_cue"))
                    .replace("{index}", &(i + 1).to_string()),
            )
            .font(type_::TITLE)
            .color(t.text),
        )
        .child(
            text(cue_range_line(&s.core, &cue))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        // Time domain (RM21): the same window as
        // timecodes + duration at the project rate —
        // the sidecars' own notation, beside the frame
        // numbers the steppers speak.
        .child(
            text(cue_time_line(&s.core, &cue))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        // Reading feedback (RM21, A9 R9-2): CPS and
        // per-line lengths — measurement only, no
        // threshold judged (see cue_cps_line).
        .child(
            text(cue_cps_line(&s.core, &cue))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    if edit_is_live(s) {
        // The open text session: the speaker field over
        // the cue-text area (Enter applies in either,
        // like every committing field in the shell; in
        // the area Shift+Enter breaks the line instead —
        // the hint under it teaches the difference) over
        // the Apply/Cancel row. The speaker field stays
        // single-line on purpose: the model's speaker
        // rule refuses a name holding a newline, so a
        // multi-line speaker field would invite text
        // its op must refuse. The cue text carries no
        // such rule — its breaks are structure (W2-16's
        // split maths treats them as real), so its
        // editor is the kit's multi-line area (W2-99)
        // in auto_grow's bounded scroll box.
        block = block
            .child(design::section_label(
                &i18n_wire::tr(&s.core, "subtitle_speaker"),
                t,
            ))
            .child(
                design::text_field(
                    &s.subtitle_panel.speaker,
                    type_::BODY,
                    s.subtitle_panel.speaker.focused,
                    t,
                )
                .key("subtitle-speaker-field")
                .single_line()
                .w_full()
                .color(t.text)
                .on_edit(|s: &mut State| &mut s.subtitle_panel.speaker)
                .on_submit(|s: &mut State, _tx: &mut rune::Tx| apply_edit(s)),
            )
            .child(rune::auto_grow(
                design::text_area(
                    &s.subtitle_panel.text,
                    type_::BODY,
                    s.subtitle_panel.text.focused,
                    t,
                )
                .key("subtitle-text-field")
                .w_full()
                .color(t.text)
                .on_edit(|s: &mut State| &mut s.subtitle_panel.text)
                .on_submit(|s: &mut State, _tx: &mut rune::Tx| apply_edit(s)),
                TEXT_AREA_MIN_LINES,
                TEXT_AREA_MAX_LINES,
            ))
            .child(
                text(i18n_wire::tr(&s.core, "subtitle_multiline_hint"))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            )
            .child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(btn(
                        "subtitle-edit-apply",
                        &i18n_wire::tr(&s.core, "subtitle_apply"),
                        false,
                        t,
                        |s| apply_edit(s),
                    ))
                    .child(btn(
                        "subtitle-edit-cancel",
                        &i18n_wire::tr(&s.core, "subtitle_cancel"),
                        false,
                        t,
                        |s| close_edit(s),
                    )),
            );
    } else {
        // A credited cue names its speaker over its
        // text (RM21 — the speaker/credit surface: the
        // row label's prefix, spelled out here where
        // the cue is read whole; the label is the
        // existing `subtitle_speaker` key and the name
        // is the cue's own data).
        if let Some(name) = &cue.speaker {
            block = block.child(
                text(format!(
                    "{}: {}",
                    i18n_wire::tr(&s.core, "subtitle_speaker"),
                    name
                ))
                .font(type_::LABEL)
                .color(t.text_secondary),
            );
        }
        // The cue's text whole (it wraps — a cue is
        // prose, and the card is its reading width),
        // over the row that opens the editor on it.
        block = block
            .child(
                text(cue.text.clone())
                    .font(type_::BODY)
                    .color(t.text)
                    .wrap()
                    .w_full(),
            )
            .child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(btn(
                        "subtitle-edit-open",
                        &i18n_wire::tr(&s.core, "subtitle_edit"),
                        false,
                        t,
                        |s| begin_edit(s),
                    )),
            );
    }
    block = block
        // Start stepper: −/+ move the cue's first frame
        // (EditorCore::subtitle_update).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_start"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-start-dn", "−", false, t, |s| step_start(s, -1)))
                .child(design::stepper_value(&format!("{}", cue.start_frame + 1), t))
                .child(btn("subtitle-start-up", "+", false, t, |s| step_start(s, 1))),
        )
        // End stepper: the end is the half-open bound —
        // its readout is the frame number itself (the
        // last covered frame), matching the row labels.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_end"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-end-dn", "−", false, t, |s| step_end(s, -1)))
                .child(design::stepper_value(&format!("{}", cue.end_frame), t))
                .child(btn("subtitle-end-up", "+", false, t, |s| step_end(s, 1))),
        )
        // Playhead placement: start at the playhead, or
        // end just past it (the playhead becomes the
        // cue's last covered frame).
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "subtitle-start-at",
                    &i18n_wire::tr(&s.core, "subtitle_start_at"),
                    false,
                    t,
                    |s| start_at_playhead(s),
                ))
                .child(btn(
                    "subtitle-end-at",
                    &i18n_wire::tr(&s.core, "subtitle_end_at"),
                    false,
                    t,
                    |s| end_at_playhead(s),
                )),
        )
        // Nudge by time (RM21): the whole window moves
        // by a time grain — the frame steppers above
        // stay for frame precision. The labels are
        // notation, like the steppers' − / +.
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_nudge"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-nudge-m1s", "-1s", false, t, |s| {
                    nudge_selected(s, -1000)
                }))
                .child(btn("subtitle-nudge-m100", "-0.1s", false, t, |s| {
                    nudge_selected(s, -100)
                }))
                .child(btn("subtitle-nudge-p100", "+0.1s", false, t, |s| {
                    nudge_selected(s, 100)
                }))
                .child(btn("subtitle-nudge-p1s", "+1s", false, t, |s| {
                    nudge_selected(s, 1000)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_only_btn(
                    "subtitle-delete",
                    "trash-can",
                    &i18n_wire::tr(&s.core, "subtitle_delete"),
                    26.0,
                    false,
                    t,
                    |s| delete_selected(s),
                )),
        );
    block
}

/// The style block: position and font-scale steppers
/// and the text/box colour swatches, over a readout
/// naming both stored colours (a colour outside the
/// palette — set over MCP — still reads here even
/// though no swatch marks it). Every press is one
/// `set_subtitle_style` call: one undo step.
/// One RGB channel stepper row of the style block
/// (Wave V, V6): the channel's name, a mini-swatch of
/// the channel's own intensity, the − button, the
/// value in the kit's `stepper_value` readout and the
/// + button — the style inspector's channel-row
/// idiom, stepping by [`SUBTITLE_RGB_STEP`] through
/// the style op via `step`. A direction the channel
/// cannot move (0 down, 255 up) renders inert (the
/// kit's `StepForm::Noop`). Keys are `{stem}-row` /
/// `{stem}-dn` / `{stem}-up`.
fn style_channel_row(
    stem: &str,
    label: &str,
    rgb: u32,
    channel: usize,
    step: fn(&mut State, usize, i32),
    t: &Theme,
) -> El {
    let value = channel_value(rgb, channel);
    let swatch = el()
        .size(12.0, 12.0)
        .radius(3.0)
        .fill(crate::stage_view::hex_color(channel_preview(value, channel)).unwrap_or(t.bg));
    row()
        .key(format!("{stem}-row"))
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(swatch)
        .child(design::step_btn(
            &format!("{stem}-dn"),
            "−",
            if value == 0 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            move |s: &mut State| step(s, channel, -1),
        ))
        .child(design::stepper_value(&value.to_string(), t))
        .child(design::step_btn(
            &format!("{stem}-up"),
            "+",
            if value == 255 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            move |s: &mut State| step(s, channel, 1),
        ))
}

fn style_block(s: &State, t: &Theme) -> El {
    let style = s.core.subtitle_style();
    // The channel rows' names, resolved once (Wave V,
    // V6 — IA-V6-1's keys).
    let channel_names = [
        i18n_wire::tr(&s.core, "subtitle_red"),
        i18n_wire::tr(&s.core, "subtitle_green"),
        i18n_wire::tr(&s.core, "subtitle_blue"),
    ];
    let mut text_colors = row().key("subtitle-text-colors").gap(4.0).center_y();
    for (ci, &rgb) in SUBTITLE_PALETTE.iter().enumerate() {
        text_colors = text_colors.child(design::swatch(
            &format!("subtitle-text-color-{ci}"),
            crate::stage_view::hex_color(rgb).unwrap_or(t.bg),
            style.text_color == rgb,
            t,
            move |s: &mut State| set_text_color(s, rgb),
        ));
    }
    let mut box_colors = row().key("subtitle-box-colors").gap(4.0).center_y();
    for (ci, &rgb) in SUBTITLE_PALETTE.iter().enumerate() {
        box_colors = box_colors.child(design::swatch(
            &format!("subtitle-box-color-{ci}"),
            crate::stage_view::hex_color(rgb).unwrap_or(t.bg),
            style.box_color == rgb,
            t,
            move |s: &mut State| set_box_color(s, rgb),
        ));
    }
    col()
        .key("subtitle-style")
        .w_full()
        .gap(8.0)
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "subtitle_style"),
            t,
        ))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_position"), t)
                        .grow(1.0),
                )
                .child(icon_only_btn(
                    "subtitle-position-prev",
                    "chevron-left",
                    &i18n_wire::tr(&s.core, "subtitle_position_prev"),
                    26.0,
                    false,
                    t,
                    |s| cycle_position(s),
                ))
                .child(design::stepper_value(
                    &position_label(&s.core, style.position),
                    t,
                ))
                .child(icon_only_btn(
                    "subtitle-position-next",
                    "chevron-right",
                    &i18n_wire::tr(&s.core, "subtitle_position_next"),
                    26.0,
                    false,
                    t,
                    |s| cycle_position(s),
                )),
        )
        // The font cycler (RM21 — IA-RM2-3's panel
        // door): the position cycler's exact shape over
        // the style's font field; the readout names the
        // renderer the burn-in will use.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_font"), t)
                        .grow(1.0),
                )
                .child(icon_only_btn(
                    "subtitle-font-prev",
                    "chevron-left",
                    &i18n_wire::tr(&s.core, "subtitle_font_prev"),
                    26.0,
                    false,
                    t,
                    |s| cycle_font(s),
                ))
                .child(design::stepper_value(
                    &font_label(&s.core, style.font),
                    t,
                ))
                .child(icon_only_btn(
                    "subtitle-font-next",
                    "chevron-right",
                    &i18n_wire::tr(&s.core, "subtitle_font_next"),
                    26.0,
                    false,
                    t,
                    |s| cycle_font(s),
                )),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_font_scale"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-scale-dn", "−", false, t, |s| step_scale(s, -1)))
                .child(design::stepper_value(&scale_readout(&s.core, style), t))
                .child(btn("subtitle-scale-up", "+", false, t, |s| step_scale(s, 1))),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(design::section_label(
                    &i18n_wire::tr(&s.core, "subtitle_text"),
                    t,
                ))
                .child(text_colors),
        )
        .child(style_channel_row(
            "subtitle-text-r",
            &channel_names[0],
            style.text_color,
            0,
            step_text_channel,
            t,
        ))
        .child(style_channel_row(
            "subtitle-text-g",
            &channel_names[1],
            style.text_color,
            1,
            step_text_channel,
            t,
        ))
        .child(style_channel_row(
            "subtitle-text-b",
            &channel_names[2],
            style.text_color,
            2,
            step_text_channel,
            t,
        ))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(design::section_label(
                    &i18n_wire::tr(&s.core, "subtitle_box"),
                    t,
                ))
                .child(box_colors),
        )
        .child(style_channel_row(
            "subtitle-box-r",
            &channel_names[0],
            style.box_color,
            0,
            step_box_channel,
            t,
        ))
        .child(style_channel_row(
            "subtitle-box-g",
            &channel_names[1],
            style.box_color,
            1,
            step_box_channel,
            t,
        ))
        .child(style_channel_row(
            "subtitle-box-b",
            &channel_names[2],
            style.box_color,
            2,
            step_box_channel,
            t,
        ))
        .child(
            text(
                i18n_wire::tr(&s.core, "subtitle_color_readout")
                    .replace("{text}", &i18n_wire::tr(&s.core, "subtitle_text"))
                    .replace("{text_hex}", &hex_label(style.text_color))
                    .replace("{box}", &i18n_wire::tr(&s.core, "subtitle_box"))
                    .replace("{box_hex}", &hex_label(style.box_color)),
            )
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
        )
}

/// The file block: the path field over the export and
/// import button rows. The format rides the BUTTON,
/// never the path's extension — the two converters'
/// refusal disciplines differ, and a `.srt` name on a
/// WebVTT document must not smuggle it past the
/// header rule.
fn file_block(s: &State, t: &Theme) -> El {
    col()
        .key("subtitle-file")
        .w_full()
        .gap(8.0)
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "subtitle_file"),
            t,
        ))
        .child(
            design::text_field(
                &s.subtitle_panel.path,
                type_::BODY,
                s.subtitle_panel.path.focused,
                t,
            )
            .key("subtitle-path")
            .single_line()
            .w_full()
            .color(t.text)
            .placeholder(
                &i18n_wire::tr(&s.core, "subtitle_path_hint"),
                t.text_secondary,
            )
            .on_edit(|s: &mut State| &mut s.subtitle_panel.path),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "subtitle-export-srt",
                    &i18n_wire::tr(&s.core, "subtitle_export_srt"),
                    false,
                    t,
                    |s| export_file(s, SubtitleFileFormat::Srt),
                ))
                .child(btn(
                    "subtitle-export-vtt",
                    &i18n_wire::tr(&s.core, "subtitle_export_vtt"),
                    false,
                    t,
                    |s| export_file(s, SubtitleFileFormat::Vtt),
                )),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "subtitle-import-srt",
                    &i18n_wire::tr(&s.core, "subtitle_import_srt"),
                    false,
                    t,
                    |s| import_file(s, SubtitleFileFormat::Srt),
                ))
                .child(btn(
                    "subtitle-import-vtt",
                    &i18n_wire::tr(&s.core, "subtitle_import_vtt"),
                    false,
                    t,
                    |s| import_file(s, SubtitleFileFormat::Vtt),
                )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::subtitles::SubtitleStyle;

    fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    /// A shell state whose project carries `cues` at
    /// 24 fps (set explicitly, the timing-test rule)
    /// over a 30-frame timeline (so `goto` has somewhere
    /// to land — the lane tests' fixture shape).
    fn cue_state(cues: Vec<SubtitleCue>) -> State {
        let mut s = crate::init();
        s.core.project.fps = 24.0;
        s.core.project.frames = (0..30)
            .map(|_| stykanimate_rs::model::Frame {
                poses: Vec::new(),
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            })
            .collect();
        s.core.project.subtitles = cues;
        s
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-subtitle-panel-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn panel_state_starts_empty_and_closed() {
        let st = SubtitlePanelState::new();
        assert_eq!(st.path.text, "");
        assert_eq!(st.text.text, "");
        assert_eq!(st.text_target, None);
        assert_eq!(st.text_opened, "");
        assert_eq!(st.retime_from, 0.0, "the retime source starts unset");
        // W2-16: the assistant's parameters start at
        // the model's defaults.
        assert_eq!(st.timing_target_cps, DEFAULT_TARGET_CPS);
        assert_eq!(st.timing_min_seconds, DEFAULT_MIN_SECONDS);
        let st = SubtitlePanelState::default();
        assert_eq!(st.text_target, None);
    }

    #[test]
    fn labels_speak_the_ops_voice() {
        let s = cue_state(Vec::new());
        assert_eq!(cue_row_label(&s.core, &cue(0, 24, "Hello.")), "1–24 · Hello.");
        assert_eq!(
            cue_row_label(&s.core, &cue(4, 6, "one\ntwo")),
            "5–6 · one two",
            "line breaks flatten for the one-line row"
        );
        // A credited cue's row carries the renderers'
        // speaker prefix inside the template's text
        // slot (Wave V, V2 composed spot check).
        let mut credited = cue(0, 24, "Hello.");
        credited.speaker = Some("NOVA".to_string());
        assert_eq!(
            cue_row_label(&s.core, &credited),
            "1–24 · NOVA: Hello."
        );
        assert_eq!(
            cue_range_line(&s.core, &cue(0, 24, "x")),
            "Frames 1–24 · 24 frame(s)"
        );
        assert_eq!(
            cue_range_line(&s.core, &cue(3, 4, "x")),
            "Frames 4–4 · 1 frame(s)"
        );
        assert_eq!(hex_label(0xFFFFFF), "#FFFFFF");
        assert_eq!(hex_label(0x000000), "#000000");
        assert_eq!(hex_label(0xFFD94D), "#FFD94D");
    }

    #[test]
    fn row_label_flattens_a_credited_multiline_cue_with_its_prefix() {
        // Roadmap Wave 2 (W2-17) consistency pin: the
        // last label combination — credited AND
        // multi-line. The row's text is the lane label
        // (`subtitle_lane::block_label`, the renderers'
        // composition) with its line breaks flattened
        // to spaces, inside the row template: one cue,
        // one reading, on every surface.
        let s = cue_state(Vec::new());
        let mut credited = cue(0, 24, "one\ntwo");
        credited.speaker = Some("NOVA".to_string());
        assert_eq!(cue_row_label(&s.core, &credited), "1–24 · NOVA: one two");
        assert_eq!(
            cue_row_label(&s.core, &credited),
            format!(
                "1–24 · {}",
                crate::rune_sec::subtitle_lane::block_label(&credited).replace('\n', " ")
            )
        );
    }

    #[test]
    fn selected_cue_resolves_only_a_live_selection() {
        let s = cue_state(vec![cue(0, 4, "a"), cue(4, 8, "b")]);
        assert_eq!(selected_cue(&s.core, None), None);
        assert_eq!(selected_cue(&s.core, Some(2)), None, "stale is no selection");
        let (i, c) = selected_cue(&s.core, Some(1)).expect("live selection");
        assert_eq!(i, 1);
        assert_eq!(c.text, "b");
    }

    #[test]
    fn steppers_hold_the_model_bounds() {
        let c = cue(0, 1, "x");
        assert_eq!(stepped_start(&c, -1), None, "frame 0 is the floor");
        assert_eq!(stepped_start(&c, 1), None, "the start must stay before the end");
        assert_eq!(stepped_end(&c, -1), None, "one frame is the minimum");
        assert_eq!(stepped_end(&c, 1), Some(2), "the end has no upper bound");
        let c = cue(4, 9, "x");
        assert_eq!(stepped_start(&c, -1), Some(3));
        assert_eq!(stepped_start(&c, 1), Some(5));
        assert_eq!(stepped_end(&c, -1), Some(8));
        assert_eq!(stepped_end(&c, 1), Some(10));
    }

    #[test]
    fn scale_steps_in_tenths_and_holds_the_clamp() {
        let s = cue_state(Vec::new());
        let style = SubtitleStyle::default();
        assert_eq!(stepped_scale(style, 1), 1.1);
        assert_eq!(stepped_scale(style, -1), 0.9);
        assert_eq!(scale_readout(&s.core, style), "1.0×");
        let lo = SubtitleStyle {
            font_scale: SUBTITLE_FONT_SCALE_MIN,
            ..SubtitleStyle::default()
        };
        assert_eq!(stepped_scale(lo, -1), SUBTITLE_FONT_SCALE_MIN);
        let hi = SubtitleStyle {
            font_scale: SUBTITLE_FONT_SCALE_MAX,
            ..SubtitleStyle::default()
        };
        assert_eq!(stepped_scale(hi, 1), SUBTITLE_FONT_SCALE_MAX);
        assert_eq!(scale_readout(&s.core, hi), "3.0×");
        // An off-grid stored scale steps a full tenth
        // from its nearest tenth (1.25 reads as 1.3 on
        // the grid, so + lands on 1.4 and − on 1.2).
        let odd = SubtitleStyle {
            font_scale: 1.25,
            ..SubtitleStyle::default()
        };
        assert_eq!(stepped_scale(odd, 1), 1.4);
        assert_eq!(stepped_scale(odd, -1), 1.2);
    }

    #[test]
    fn subtitle_i18n_keys_exist_and_match_the_english_labels() {
        // Every key the panel renders through tr()
        // exists in the catalogue, and a default
        // (English) core resolves it to the exact
        // literal it replaced — English rendering is
        // byte-identical to the pre-i18n panel.
        const LABELS: [(&str, &str); 52] = [
            ("subtitles", "Subtitles"),
            ("subtitle_none", "No subtitles in this project."),
            ("subtitle_add", "Add cue"),
            ("subtitle_select", "Select a subtitle cue to edit it."),
            ("subtitle_cue", "Cue"),
            ("frames", "Frames"),
            ("subtitle_frames_unit", "frame(s)"),
            ("subtitle_new", "New subtitle"),
            ("subtitle_edit", "Edit text"),
            ("subtitle_apply", "Apply text"),
            ("subtitle_cancel", "Cancel"),
            ("subtitle_start", "Start"),
            ("subtitle_end", "End"),
            ("subtitle_start_at", "Start @ playhead"),
            ("subtitle_end_at", "End @ playhead"),
            ("subtitle_delete", "Delete cue"),
            ("subtitle_style", "Style"),
            ("subtitle_position", "Position"),
            ("subtitle_position_prev", "Previous position"),
            ("subtitle_position_next", "Next position"),
            ("subtitle_bottom", "Bottom"),
            ("subtitle_top", "Top"),
            ("subtitle_font_scale", "Font scale"),
            ("subtitle_text", "Text"),
            ("subtitle_box", "Box"),
            ("subtitle_file", "File"),
            (
                "subtitle_path_hint",
                "Subtitle file path — e.g. subtitles.srt",
            ),
            ("subtitle_export_srt", "Export SRT"),
            ("subtitle_export_vtt", "Export VTT"),
            ("subtitle_import_srt", "Import SRT"),
            ("subtitle_import_vtt", "Import VTT"),
            // Wave V (V6): the channel rows' names —
            // these three resolve only once IA-V6-1
            // lands the keys in the catalogue (the
            // integrator's insertion, roster order).
            ("subtitle_red", "Red"),
            ("subtitle_green", "Green"),
            ("subtitle_blue", "Blue"),
            // Roadmap Wave 1 (RM21 — IA-RM21-2): the
            // font cycler, the nudge row, the Timing
            // block and the lane's empty line. Like the
            // V6 keys above, these resolve only once the
            // IA lands the keys in the catalogue.
            ("subtitle_font", "Font"),
            ("subtitle_font_prev", "Previous font"),
            ("subtitle_font_next", "Next font"),
            ("subtitle_font_shaped", "Shaped"),
            ("subtitle_font_pixel", "Pixel"),
            ("subtitle_nudge", "Nudge"),
            ("subtitle_timing", "Timing"),
            ("subtitle_shift_all", "Shift all cues"),
            ("subtitle_retime", "Retime from (fps)"),
            ("subtitle_retime_apply", "Retime cues"),
            (
                "subtitle_lane_empty",
                "No subtitles yet — add the first cue from here.",
            ),
            // Roadmap Wave 2 (W2-16 — IA-W2-16-3): the
            // Timing assistant block's labels. Like the
            // RM21 keys above, these resolve only once
            // the IA lands the keys in the catalogue.
            ("subtitle_assist", "Timing assistant"),
            ("subtitle_assist_target", "Target (chars/s)"),
            ("subtitle_assist_min", "Minimum (s)"),
            ("subtitle_assist_apply", "Apply"),
            ("subtitle_assist_apply_all", "Apply all"),
            (
                "subtitle_assist_none",
                "Every cue fits the target speed.",
            ),
            // Roadmap Wave 2 (W2-99 — IA-W2-99-1): the
            // multi-line editor's hint line. Like the
            // RM21 keys above, it resolves only once
            // the IA lands the key in the catalogue.
            (
                "subtitle_multiline_hint",
                "Shift+Enter starts a new line; Enter applies.",
            ),
        ];
        let s = cue_state(Vec::new());
        for (key, english) in LABELS {
            assert!(
                i18n_wire::key_in_table(key),
                "{key} missing from the catalogue"
            );
            assert_eq!(i18n_wire::tr(&s.core, key), english, "{key} in English");
        }
        // The composed readouts' template keys (Wave
        // V, V2 — IA-V2-1): present in the catalogue
        // and resolving in English to their stored
        // template, slots intact. This section passes
        // only after IA-V2-1 lands — until then tr()
        // falls back to the key itself (the Q7
        // pattern). The composed spot checks through
        // the public helpers live in
        // labels_speak_the_ops_voice (row + range
        // lines) and scale_steps_in_tenths (the scale
        // value); the cue title and colour line
        // compose inline in their views and are
        // pinned by their stored templates here.
        const TEMPLATES: [(&str, &str); 12] = [
            ("subtitle_row_label", "{start}–{end} · {text}"),
            ("subtitle_cue_title", "{cue} {index}"),
            (
                "subtitle_range_line",
                "{frames} {start}–{end} · {count} {unit}",
            ),
            ("subtitle_scale_value", "{value}×"),
            (
                "subtitle_color_readout",
                "{text} {text_hex} · {box} {box_hex}",
            ),
            // Roadmap Wave 1 (RM21 — IA-RM21-2): the
            // detail block's time and feedback lines.
            (
                "subtitle_time_line",
                "{start} → {end} · {seconds} s",
            ),
            (
                "subtitle_cps_line",
                "{cps} chars/s · lines {lines}",
            ),
            // Roadmap Wave 2 (W2-16 — IA-W2-16-3): the
            // Timing assistant's proposal lines and its
            // count lines.
            (
                "subtitle_assist_extend",
                "{cue} {index} · extend to {seconds} s · {cps} → {after} chars/s",
            ),
            (
                "subtitle_assist_split",
                "{cue} {index} · split · {cps} → {after} chars/s",
            ),
            (
                "subtitle_assist_split_shift",
                "{cue} {index} · split + shift {shift} f · {cps} → {after} chars/s",
            ),
            ("subtitle_assist_count", "{count} suggestion(s)"),
            (
                "subtitle_assist_stuck",
                "{count} cue(s) have no room to grow",
            ),
        ];
        for (key, template) in TEMPLATES {
            assert!(
                i18n_wire::key_in_table(key),
                "{key} missing from the catalogue"
            );
            assert_eq!(i18n_wire::tr(&s.core, key), template, "{key} template");
        }
        assert_eq!(DEFAULT_CUE_TEXT, i18n_wire::tr(&s.core, "subtitle_new"));
        // The translated tables genuinely translate:
        // the section title and the position readout
        // follow the core's language.
        let mut s = cue_state(Vec::new());
        s.core.lang = stykanimate_rs::i18n::Lang::Bn;
        assert_eq!(i18n_wire::tr(&s.core, "subtitles"), "সাবটাইটেল");
        assert_eq!(
            cue_range_line(&s.core, &cue(0, 24, "x")),
            "ফ্রেমসমূহ 1–24 · 24 ফ্রেম"
        );
        s.core.lang = stykanimate_rs::i18n::Lang::De;
        assert_eq!(position_label(&s.core, SubtitlePosition::Bottom), "Unten");
        assert_eq!(position_label(&s.core, SubtitlePosition::Top), "Oben");
    }

    #[test]
    fn position_cycles_between_the_two() {
        let s = cue_state(Vec::new());
        assert_eq!(
            position_label(&s.core, SubtitlePosition::Bottom),
            "Bottom"
        );
        assert_eq!(position_label(&s.core, SubtitlePosition::Top), "Top");
        assert_eq!(flipped_position(SubtitlePosition::Bottom), SubtitlePosition::Top);
        assert_eq!(flipped_position(SubtitlePosition::Top), SubtitlePosition::Bottom);
    }

    #[test]
    fn palette_is_eight_distinct_colours_with_white_and_black() {
        let mut sorted: Vec<u32> = SUBTITLE_PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "every swatch colour is distinct");
        assert!(SUBTITLE_PALETTE.contains(&0xFFFFFF));
        assert!(SUBTITLE_PALETTE.contains(&0x000000));
    }

    #[test]
    fn add_cue_lands_at_the_playhead_selected_with_the_editor_open() {
        let mut s = cue_state(Vec::new());
        s.core.goto(5);
        let depth = s.core.undo_depth();
        add_cue(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1, "one undo step for the add");
        assert_eq!(s.core.subtitle_count(), 1);
        let c = &s.core.project.subtitles[0];
        assert_eq!((c.start_frame, c.end_frame), (5, 29), "one second at 24 fps");
        assert_eq!(c.text, DEFAULT_CUE_TEXT);
        assert_eq!(s.subtitle_selected, Some(0));
        assert_eq!(s.subtitle_panel.text_target, Some(0));
        assert_eq!(s.subtitle_panel.text_opened, DEFAULT_CUE_TEXT);
        assert!(edit_is_live(&s));
    }

    #[test]
    fn steppers_move_the_cue_and_the_selection_follows_the_resort() {
        let mut s = cue_state(vec![cue(0, 4, "a"), cue(10, 14, "b")]);
        s.subtitle_selected = Some(0);
        let depth = s.core.undo_depth();
        // Widen cue a past cue b's window, then step its
        // start past b's start: the update re-sorts, and
        // the selection must follow the cue, not the
        // index it used to sit at.
        for _ in 0..12 {
            step_end(&mut s, 1);
        }
        assert_eq!(s.subtitle_selected, Some(0), "still first while its start leads");
        for _ in 0..12 {
            step_start(&mut s, 1);
        }
        assert_eq!(s.core.undo_depth(), depth + 24, "one step per press");
        let sel = s.subtitle_selected.expect("selection survives");
        assert_eq!(sel, 1, "the cue re-sorted behind b");
        assert_eq!(s.core.project.subtitles[sel].text, "a");
        // End stepping at the one-frame minimum refuses
        // without a step.
        let mut s = cue_state(vec![cue(3, 4, "x")]);
        s.subtitle_selected = Some(0);
        let depth = s.core.undo_depth();
        step_end(&mut s, -1);
        assert_eq!(s.core.undo_depth(), depth, "a bound press spends nothing");
        assert!(s.core.status.contains("one frame long"));
    }

    #[test]
    fn playhead_placement_moves_and_refuses_in_voice() {
        let mut s = cue_state(vec![cue(10, 20, "x")]);
        s.subtitle_selected = Some(0);
        s.core.goto(12);
        start_at_playhead(&mut s);
        assert_eq!(s.core.project.subtitles[0].start_frame, 12);
        end_at_playhead(&mut s);
        assert_eq!(s.core.project.subtitles[0].end_frame, 13);
        // Playhead before the cue: the end move refuses.
        s.core.goto(0);
        let depth = s.core.undo_depth();
        end_at_playhead(&mut s);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("Subtitle end move failed"));
    }

    #[test]
    fn delete_clears_the_selection_and_the_editor() {
        let mut s = cue_state(vec![cue(0, 4, "a"), cue(4, 8, "b")]);
        s.subtitle_selected = Some(0);
        begin_edit(&mut s);
        assert!(edit_is_live(&s));
        let depth = s.core.undo_depth();
        delete_selected(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1);
        assert_eq!(s.core.subtitle_count(), 1);
        assert_eq!(s.subtitle_selected, None);
        assert_eq!(s.subtitle_panel.text_target, None);
    }

    #[test]
    fn apply_edit_writes_once_and_an_unchanged_buffer_is_free() {
        let mut s = cue_state(vec![cue(0, 24, "Old line")]);
        s.subtitle_selected = Some(0);
        begin_edit(&mut s);
        let depth = s.core.undo_depth();
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "unchanged text spends no step");
        assert_eq!(s.subtitle_panel.text_target, None, "the session closed");

        begin_edit(&mut s);
        s.subtitle_panel.text = TextEdit::new("New line");
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1, "one step for the rewrite");
        assert_eq!(s.core.project.subtitles[0].text, "New line");
        assert_eq!(s.subtitle_panel.text_target, None);
    }

    #[test]
    fn apply_edit_refuses_when_the_world_moved_under_the_editor() {
        let mut s = cue_state(vec![cue(0, 24, "Original")]);
        s.subtitle_selected = Some(0);
        begin_edit(&mut s);
        // Another surface rewrites the cue (an undo, a
        // lane edit, the live MCP server): the index
        // now carries different text than the session
        // opened with.
        s.core.project.subtitles[0].text = "Rewritten elsewhere".to_string();
        s.subtitle_panel.text = TextEdit::new("Typed over it");
        let depth = s.core.undo_depth();
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "a refused apply spends nothing");
        assert_eq!(s.core.project.subtitles[0].text, "Rewritten elsewhere");
        assert_eq!(s.subtitle_panel.text_target, None, "the session stood down");
        assert!(s.core.status.contains("Subtitle text edit failed"));
    }

    #[test]
    fn multiline_text_round_trips_verbatim_through_the_editor() {
        // W2-99: the buffer IS the cue's text whole —
        // line breaks, a blank line and trailing spaces
        // are content, not formatting. An unedited
        // multi-line buffer applies for free (the
        // unchanged check compares the whole string);
        // an edited one writes back byte-for-byte,
        // trailing newline included, in one undo step.
        let original = "First line  \n\nThird line";
        let mut s = cue_state(vec![cue(0, 24, original)]);
        s.subtitle_selected = Some(0);
        begin_edit(&mut s);
        assert_eq!(s.subtitle_panel.text.text, original);
        let depth = s.core.undo_depth();
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "unchanged text spends no step");
        assert_eq!(s.core.project.subtitles[0].text, original);

        begin_edit(&mut s);
        let edited = "Edited  \n\nlast line\n";
        s.subtitle_panel.text = TextEdit::new(edited);
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1, "one step for the rewrite");
        assert_eq!(
            s.core.project.subtitles[0].text, edited,
            "the stored text is the buffer, byte-for-byte"
        );
        assert_eq!(s.subtitle_panel.text_target, None, "the session closed");
    }

    #[test]
    fn breaks_only_buffer_refuses_and_the_session_stays_open() {
        // The editor refuses exactly where the model
        // does: a buffer of spaces and line breaks is
        // empty after trim (SubtitleCue::validate), so
        // the op refuses, its status stands, nothing is
        // written, no step is spent — and the session
        // stays OPEN so the text can be fixed in place
        // (apply_edit's documented third guard).
        let mut s = cue_state(vec![cue(0, 24, "Real text")]);
        s.subtitle_selected = Some(0);
        begin_edit(&mut s);
        s.subtitle_panel.text = TextEdit::new(" \n \n");
        let depth = s.core.undo_depth();
        apply_edit(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "a refused apply spends nothing");
        assert_eq!(s.core.project.subtitles[0].text, "Real text");
        assert_eq!(
            s.subtitle_panel.text_target,
            Some(0),
            "the session stays open on an op refusal"
        );
        assert!(s.core.status.contains("Subtitle update failed: text is empty"));
    }

    #[test]
    fn section_constructs_with_the_editor_open_on_a_multiline_cue() {
        // W2-99's construction branch (the house
        // discipline: element trees are not readable
        // from unit tests, construction is the
        // assertable level — the presented-tree proof
        // belongs to CI): the multi-line area and its
        // auto_grow scroll box build on a cue whose
        // text carries breaks, under both themes.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let mut s = cue_state(vec![cue(0, 24, "one\ntwo\nthree")]);
            s.subtitle_selected = Some(0);
            begin_edit(&mut s);
            let _ = subtitle_section(&s, &t);
        }
    }

    #[test]
    fn style_controls_spend_one_step_and_hold_at_the_bounds() {
        let mut s = cue_state(Vec::new());
        let depth = s.core.undo_depth();
        cycle_position(&mut s);
        assert_eq!(s.core.subtitle_style().position, SubtitlePosition::Top);
        assert_eq!(s.core.undo_depth(), depth + 1);
        step_scale(&mut s, 1);
        assert_eq!(s.core.subtitle_style().font_scale, 1.1);
        set_text_color(&mut s, 0xFFD94D);
        assert_eq!(s.core.subtitle_style().text_color, 0xFFD94D);
        assert_eq!(s.core.undo_depth(), depth + 3);
        // Pressing the colour already in force is free.
        set_text_color(&mut s, 0xFFD94D);
        set_box_color(&mut s, 0x000000);
        assert_eq!(s.core.undo_depth(), depth + 3);
        // The scale floor reports instead of spending.
        for _ in 0..10 {
            step_scale(&mut s, -1);
        }
        assert_eq!(s.core.subtitle_style().font_scale, SUBTITLE_FONT_SCALE_MIN);
        let depth = s.core.undo_depth();
        step_scale(&mut s, -1);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("smallest"));
    }

    #[test]
    fn channel_stepping_moves_one_channel_by_the_grain() {
        // Wave V (V6): the pure stepper behind the
        // channel rows.
        assert_eq!(channel_value(0x123456, 0), 0x12);
        assert_eq!(channel_value(0x123456, 1), 0x34);
        assert_eq!(channel_value(0x123456, 2), 0x56);
        assert_eq!(stepped_channel(0x123456, 0, 1), 0x1A3456);
        assert_eq!(stepped_channel(0x123456, 1, -1), 0x122C56);
        assert_eq!(stepped_channel(0x123456, 2, 1), 0x12345E);
        // The clamps hold and the other channels never
        // move.
        assert_eq!(stepped_channel(0xFF3456, 0, 1), 0xFF3456);
        assert_eq!(stepped_channel(0x003456, 0, -1), 0x003456);
        assert_eq!(stepped_channel(0x1234FE, 2, 1), 0x1234FF);
        // A bad channel index is identity.
        assert_eq!(stepped_channel(0x123456, 3, 1), 0x123456);
        assert_eq!(channel_value(0x123456, 3), 0);
        // The mini-swatch previews carry the channel's
        // own value in its own slot.
        assert_eq!(channel_preview(0xAB, 0), 0xAB0000);
        assert_eq!(channel_preview(0xAB, 1), 0x00AB00);
        assert_eq!(channel_preview(0xAB, 2), 0x0000AB);
    }

    #[test]
    fn style_channel_steppers_round_trip_and_persist() {
        // Wave V (V6): the channel rows' handlers,
        // against a colour no swatch carries (the gap
        // the audit found: an MCP-set colour the panel
        // could display but never adjust).
        let mut s = cue_state(Vec::new());
        let off_palette = SubtitleStyle {
            text_color: 0x123456,
            box_color: 0x000000,
            ..SubtitleStyle::default()
        };
        s.core.set_subtitle_style(off_palette).unwrap();
        let depth = s.core.undo_depth();
        // One press: exactly one channel moves by the
        // grain, exactly one undo step is spent.
        step_text_channel(&mut s, 1, 1);
        assert_eq!(s.core.subtitle_style().text_color, 0x123C56);
        assert_eq!(s.core.undo_depth(), depth + 1);
        step_box_channel(&mut s, 0, 1);
        assert_eq!(s.core.subtitle_style().box_color, 0x080000);
        assert_eq!(s.core.undo_depth(), depth + 2);
        // A press at the clamp bound spends nothing,
        // and neither does a bad channel index.
        step_box_channel(&mut s, 2, -1);
        step_text_channel(&mut s, 7, 1);
        assert_eq!(s.core.undo_depth(), depth + 2);
        assert_eq!(s.core.subtitle_style().box_color, 0x080000);
        // The steppers walk a colour ONTO a palette
        // value from off-palette (caption yellow's red
        // channel is already 0xFF; its green walks
        // 0xD9 from 0xD1 in one press).
        let mut s = cue_state(Vec::new());
        s.core
            .set_subtitle_style(SubtitleStyle {
                text_color: 0xFFD100,
                ..SubtitleStyle::default()
            })
            .unwrap();
        step_text_channel(&mut s, 1, 1);
        assert_eq!(s.core.subtitle_style().text_color, 0xFFD900);
        step_text_channel(&mut s, 2, 1);
        assert_eq!(s.core.subtitle_style().text_color, 0xFFD908);
        // Persistence: the stepped style is the stored
        // project field — it survives the project's
        // own serialisation round-trip verbatim.
        let json = serde_json::to_string(&s.core.project).unwrap();
        let back: stykanimate_rs::model::Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.subtitle_style, s.core.subtitle_style());
        // And the step is a real undo step: undoing
        // restores the colour the press started from.
        s.core.undo();
        assert_eq!(s.core.subtitle_style().text_color, 0xFFD900);
    }

    #[test]
    fn style_block_constructs_with_an_off_palette_colour() {
        // Construction-level (the kit's discipline, as
        // in section_constructs_on_every_state_branch):
        // the channel rows render for a colour that
        // marks no swatch, on both themes.
        let mut s = cue_state(Vec::new());
        s.core
            .set_subtitle_style(SubtitleStyle {
                text_color: 0x123456,
                box_color: 0xABCDEF,
                ..SubtitleStyle::default()
            })
            .unwrap();
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let _ = subtitle_section(&s, &t);
        }
    }

    #[test]
    fn export_writes_the_document_and_import_replaces_once() {
        let dir = temp_dir("round-trip");
        let path = dir.join("cues.srt");
        let mut s = cue_state(vec![cue(0, 24, "First line"), cue(24, 48, "Second line")]);
        s.subtitle_panel.path = TextEdit::new(path.to_string_lossy().into_owned());
        let depth = s.core.undo_depth();
        export_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(s.core.undo_depth(), depth, "export is read-only");
        let written = std::fs::read_to_string(&path).expect("the file was written");
        assert_eq!(written, subtitles_to_srt(&s.core.project));

        // Importing the same document back is the
        // identical import: free, and it says so.
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("nothing changed"));

        // A different document replaces in one step and
        // stands the selection down.
        std::fs::write(
            &path,
            "1\n00:00:02,000 --> 00:00:03,000\nOnly cue\n",
        )
        .unwrap();
        s.subtitle_selected = Some(0);
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(s.core.undo_depth(), depth + 1);
        assert_eq!(s.core.subtitle_count(), 1);
        assert_eq!(s.core.project.subtitles[0].text, "Only cue");
        assert_eq!(s.subtitle_selected, None);
    }

    #[test]
    fn srt_export_status_reports_dropped_speaker_credits() {
        // Wave S (S9): the SRT success line names the
        // speaker credits the document could not write;
        // an uncredited export's line is the established
        // sentence, byte for byte, and WebVTT — which
        // keeps speakers — never grows the clause.
        let dir = temp_dir("srt-speakers");
        let path = dir.join("cues.srt");
        let mut credited = cue(0, 24, "First line");
        credited.speaker = Some("Mary Jane".to_string());
        let mut s = cue_state(vec![credited, cue(24, 48, "Second line")]);
        s.subtitle_panel.path = TextEdit::new(path.to_string_lossy().into_owned());
        export_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(
            s.core.status,
            format!(
                "Exported 2 subtitle cue(s) to {} — 1 speaker credit(s) not written (SRT has no speaker syntax).",
                path.to_string_lossy()
            )
        );
        // The document itself carries no name.
        let written = std::fs::read_to_string(&path).expect("the file was written");
        assert!(!written.contains("Mary"), "no name in the SRT: {written}");

        // Uncredited: the established sentence, unchanged.
        let mut s = cue_state(vec![cue(0, 24, "First line"), cue(24, 48, "Second line")]);
        s.subtitle_panel.path = TextEdit::new(path.to_string_lossy().into_owned());
        export_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(
            s.core.status,
            format!("Exported 2 subtitle cue(s) to {}.", path.to_string_lossy())
        );

        // WebVTT keeps the speaker: established sentence
        // even for a credited project, name on the wire.
        let vtt_path = dir.join("cues.vtt");
        let mut credited = cue(0, 24, "First line");
        credited.speaker = Some("Mary Jane".to_string());
        let mut s = cue_state(vec![credited]);
        s.subtitle_panel.path = TextEdit::new(vtt_path.to_string_lossy().into_owned());
        export_file(&mut s, SubtitleFileFormat::Vtt);
        assert_eq!(
            s.core.status,
            format!("Exported 1 subtitle cue(s) to {}.", vtt_path.to_string_lossy())
        );
        let written = std::fs::read_to_string(&vtt_path).expect("the file was written");
        assert!(written.contains("<v Mary Jane>"), "the voice tag is written: {written}");
    }

    #[test]
    fn import_refusals_change_nothing() {
        let dir = temp_dir("refuse");
        let mut s = cue_state(vec![cue(0, 24, "Kept")]);
        let depth = s.core.undo_depth();
        // No path at all.
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert!(s.core.status.contains("no file path"));
        // An unreadable path.
        let missing = dir.join("nope.srt");
        s.subtitle_panel.path = TextEdit::new(missing.to_string_lossy().into_owned());
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert!(s.core.status.contains("could not read"));
        // A zero-yield document: the parser's own error.
        let junk = dir.join("junk.srt");
        std::fs::write(&junk, "nothing parseable\n").unwrap();
        s.subtitle_panel.path = TextEdit::new(junk.to_string_lossy().into_owned());
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert!(s.core.status.contains("Subtitle import failed"));
        // A headerless WebVTT: the whole-file refusal.
        let headerless = dir.join("junk.vtt");
        std::fs::write(&headerless, "00:00:00.000 --> 00:00:01.000\nNo header\n").unwrap();
        s.subtitle_panel.path = TextEdit::new(headerless.to_string_lossy().into_owned());
        import_file(&mut s, SubtitleFileFormat::Vtt);
        assert!(s.core.status.contains("Subtitle import failed"));
        assert_eq!(s.core.subtitle_count(), 1);
        assert_eq!(s.core.project.subtitles[0].text, "Kept");
        assert_eq!(s.core.undo_depth(), depth);
    }

    #[test]
    fn timecode_and_feedback_helpers_measure_the_cue() {
        // RM21 (A9 R9-3/R9-2): the time domain is
        // frames ÷ project fps, in the sidecars' own
        // notation; the feedback is measurement only.
        assert_eq!(timecode_of(0, 24.0), "00:00:00.000");
        assert_eq!(timecode_of(24, 24.0), "00:00:01.000");
        assert_eq!(timecode_of(36, 24.0), "00:00:01.500");
        assert_eq!(timecode_of(87_864, 24.0), "01:01:01.000");
        assert_eq!(timecode_of(24, 0.0), "00:00:00.000", "no rate, no NaN");
        let c = cue(24, 72, "x");
        assert_eq!(cue_duration_seconds(&c, 24.0), 2.0);
        let s = cue_state(Vec::new());
        assert_eq!(
            cue_time_line(&s.core, &c),
            "00:00:01.000 → 00:00:03.000 · 2.0 s"
        );
        // CPS counts characters without line breaks;
        // line lengths count per line, in characters.
        let two = cue(0, 48, "one\ntwo three");
        assert_eq!(cue_cps(&two, 24.0), 6.0, "12 chars over 2 s");
        assert_eq!(cue_line_lengths(&two), vec![3, 9]);
        assert_eq!(cue_cps_line(&s.core, &two), "6.0 chars/s · lines 3 / 9");
        assert_eq!(cue_cps(&cue(0, 24, "Hello world"), 24.0), 11.0);
    }

    #[test]
    fn nudge_maths_rounds_and_never_vanishes() {
        // RM21: a millisecond nudge becomes whole
        // frames at the project rate — and a press
        // always moves at least one frame.
        assert_eq!(nudge_delta_frames(1000, 24.0), 24);
        assert_eq!(nudge_delta_frames(-1000, 24.0), -24);
        assert_eq!(nudge_delta_frames(100, 24.0), 2);
        assert_eq!(nudge_delta_frames(-100, 24.0), -2);
        assert_eq!(nudge_delta_frames(10, 24.0), 1, "sub-frame rounds up to a move");
        assert_eq!(nudge_delta_frames(0, 24.0), 0);
        assert_eq!(nudge_delta_frames(100, 0.0), 0, "no rate, no move");
        // The whole-window shift preserves duration
        // and clamps by APPLIED delta at frame 0.
        let c = cue(10, 20, "x");
        assert_eq!(shifted_window(&c, 5), Some((15, 25)));
        assert_eq!(shifted_window(&c, -15), Some((0, 5)));
        assert_eq!(shifted_window(&c, 0), None);
        assert_eq!(shifted_window(&cue(0, 5, "x"), -1), None);
    }

    #[test]
    fn nudge_moves_the_whole_cue_and_refuses_at_the_floor() {
        let mut s = cue_state(vec![cue(24, 48, "a")]);
        s.subtitle_selected = Some(0);
        let depth = s.core.undo_depth();
        nudge_selected(&mut s, 1000);
        let c = &s.core.project.subtitles[0];
        assert_eq!((c.start_frame, c.end_frame), (48, 72), "one second at 24 fps");
        assert_eq!(s.core.undo_depth(), depth + 1, "one step per nudge");
        // A partial clamp: 12 frames of travel remain.
        let mut s = cue_state(vec![cue(12, 36, "a")]);
        s.subtitle_selected = Some(0);
        nudge_selected(&mut s, -1000);
        let c = &s.core.project.subtitles[0];
        assert_eq!((c.start_frame, c.end_frame), (0, 24), "duration preserved");
        // At the floor the press reports, spending
        // nothing.
        let mut s = cue_state(vec![cue(0, 24, "a")]);
        s.subtitle_selected = Some(0);
        let depth = s.core.undo_depth();
        nudge_selected(&mut s, -100);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("already starts at the first frame"));
    }

    #[test]
    fn shift_all_spends_one_step_and_stands_the_selection_down() {
        let mut s = cue_state(vec![cue(0, 24, "a"), cue(48, 72, "b")]);
        s.subtitle_selected = Some(1);
        let depth = s.core.undo_depth();
        shift_all(&mut s, 1000);
        assert_eq!(s.core.undo_depth(), depth + 1, "ONE step for the whole shift");
        let wins: Vec<(usize, usize)> = s
            .core
            .project
            .subtitles
            .iter()
            .map(|c| (c.start_frame, c.end_frame))
            .collect();
        assert_eq!(wins, vec![(24, 48), (72, 96)]);
        assert_eq!(s.subtitle_selected, None, "indices stood down (import discipline)");
        assert!(s.core.status.contains("Shifted 2 subtitle cue(s) by 1000 ms."));
        // A shift no cue can take is free and says so.
        let mut s = cue_state(vec![cue(0, 24, "a")]);
        let depth = s.core.undo_depth();
        shift_all(&mut s, -1000);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("no cue moved"));
        // No cues: the handler is a silent no-op (the
        // block never mounts — this is the backstop).
        let mut s = cue_state(Vec::new());
        shift_all(&mut s, 1000);
        assert_eq!(s.core.subtitle_count(), 0);
    }

    #[test]
    fn timing_steppers_stay_in_range_and_spend_nothing() {
        // W2-16: the assistant's parameters are panel
        // state — stepping them moves no cue and spends
        // no undo step; the steppers hold the model's
        // ranges at both ends.
        let mut s = cue_state(vec![cue(0, 24, "x")]);
        let depth = s.core.undo_depth();
        step_timing_target(&mut s, 1);
        assert_eq!(s.subtitle_panel.timing_target_cps, 18.0);
        step_timing_target(&mut s, -1);
        step_timing_target(&mut s, -1);
        assert_eq!(s.subtitle_panel.timing_target_cps, 16.0);
        for _ in 0..100 {
            step_timing_target(&mut s, 1);
        }
        assert_eq!(s.subtitle_panel.timing_target_cps, TARGET_CPS_MAX);
        for _ in 0..100 {
            step_timing_target(&mut s, -1);
        }
        assert_eq!(s.subtitle_panel.timing_target_cps, TARGET_CPS_MIN);
        step_timing_min(&mut s, 1);
        assert_eq!(s.subtitle_panel.timing_min_seconds, 1.25);
        for _ in 0..100 {
            step_timing_min(&mut s, -1);
        }
        assert_eq!(s.subtitle_panel.timing_min_seconds, 0.0);
        for _ in 0..100 {
            step_timing_min(&mut s, 1);
        }
        assert_eq!(s.subtitle_panel.timing_min_seconds, MIN_SECONDS_MAX);
        assert_eq!(s.core.undo_depth(), depth, "steppers spend no undo step");
        assert_eq!(s.core.subtitle_count(), 1, "and move no cue");
    }

    #[test]
    fn assist_lines_speak_the_measured_numbers() {
        // W2-16: an extend proposal's line names the
        // cue, the fix, the landing duration and the
        // pace before → after — the model's numbers,
        // nothing judged. (The template keys resolve
        // once IA-W2-16-3 lands, the Q7 pattern.)
        let s = cue_state(vec![
            cue(0, 24, "aaaa bbbb cccc dddd eeee ffff gggg"),
            cue(96, 120, "later"),
        ]);
        let report = timing_report_for(&s.core, &s.subtitle_panel);
        assert_eq!(report.proposals.len(), 1);
        assert_eq!(
            assist_proposal_line(&s.core, &report.proposals[0]),
            "Cue 1 · extend to 2.0 s · 34.0 → 17.0 chars/s"
        );
        // A comfortable list proposes nothing and the
        // report is clean (no proposals, nothing stuck).
        let s = cue_state(vec![cue(0, 48, "aaaa bbbb cccc dddd eeee ffff gggg")]);
        let report = timing_report_for(&s.core, &s.subtitle_panel);
        assert!(report.proposals.is_empty());
        assert_eq!(report.stuck, 0);
    }

    #[test]
    fn apply_one_split_spends_exactly_one_step_and_keeps_the_text() {
        // W2-16: the engine's split + shift fixture at
        // panel scale — one press, one undo step, the
        // dialogue rejoined verbatim, the selection
        // stood down. The handler rides the core op
        // filed as IA-W2-16-2, so this pin first runs
        // at integration CI (the RM21 pattern).
        let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
        let mut s = cue_state(vec![cue(0, 24, text), cue(40, 64, "next")]);
        s.subtitle_selected = Some(0);
        let depth = s.core.undo_depth();
        apply_timing_proposal(&mut s, 0);
        assert_eq!(s.core.undo_depth(), depth + 1, "one press, one undo step");
        assert_eq!(s.core.subtitle_count(), 3);
        let texts: Vec<&str> = s
            .core
            .subtitle_cues()
            .iter()
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(
            texts,
            ["aaaa bbbb cccc dddd", "eeee ffff gggg hhhh", "next"]
        );
        assert_eq!(s.subtitle_selected, None, "the selection stands down");
    }

    #[test]
    fn apply_all_spends_one_step_for_the_whole_batch() {
        // W2-16: two dense cues fixed by one press of
        // Apply-all cost ONE undo step between them.
        let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
        let mut s = cue_state(vec![
            cue(0, 24, text),
            cue(40, 64, text),
            cue(80, 104, "tail"),
        ]);
        let depth = s.core.undo_depth();
        apply_all_timing_proposals(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1, "the batch is one step");
        assert_eq!(s.core.subtitle_count(), 5);
    }

    #[test]
    fn apply_with_nothing_to_fix_spends_nothing() {
        // W2-16: an all-comfortable list reports and
        // spends nothing — no fake step for a press
        // that changed nothing.
        let mut s = cue_state(vec![cue(0, 48, "aaaa bbbb cccc dddd eeee ffff gggg")]);
        let depth = s.core.undo_depth();
        apply_all_timing_proposals(&mut s);
        assert_eq!(s.core.undo_depth(), depth);
        assert_eq!(s.core.subtitle_count(), 1);
    }

    #[test]
    fn retime_scales_windows_and_names_both_rates() {
        // RM21: retiming is explicit — the press, the
        // rescale, and a status naming both rates.
        assert_eq!(retimed_window(&cue(24, 48, "x"), 24.0, 30.0), (30, 60));
        assert_eq!(retimed_window(&cue(0, 1, "x"), 24.0, 12.0), (0, 1));
        assert_eq!(
            retimed_window(&cue(24, 48, "x"), 24.0, 24.0),
            (24, 48),
            "equal rates are the identity"
        );
        let mut s = cue_state(vec![cue(24, 48, "a")]);
        // Unset source = the project rate: a free
        // no-op report.
        let depth = s.core.undo_depth();
        apply_retime(&mut s);
        assert_eq!(s.core.undo_depth(), depth);
        assert!(s.core.status.contains("already match the project frame rate"));
        // Stepping the source is panel state only.
        step_retime_from(&mut s, -1);
        assert_eq!(s.subtitle_panel.retime_from, 23.0);
        assert_eq!(s.core.undo_depth(), depth, "stepping spends no undo step");
        // Apply: one step, windows scaled by 24/23,
        // both rates named on the status line.
        apply_retime(&mut s);
        assert_eq!(s.core.undo_depth(), depth + 1);
        let c = &s.core.project.subtitles[0];
        assert_eq!((c.start_frame, c.end_frame), (25, 50));
        assert_eq!(
            s.core.status,
            "Retimed 1 subtitle cue(s) from 23 fps to 24 fps."
        );
        // The steppers hold the 1..=120 rail.
        let mut s = cue_state(Vec::new());
        s.subtitle_panel.retime_from = 120.0;
        step_retime_from(&mut s, 1);
        assert_eq!(s.subtitle_panel.retime_from, 120.0);
        s.subtitle_panel.retime_from = 1.0;
        step_retime_from(&mut s, -1);
        assert_eq!(s.subtitle_panel.retime_from, 1.0);
    }

    #[test]
    fn font_cycler_flips_the_style_font_in_one_step() {
        // RM21 (IA-RM2-3): the panel door for RM2's
        // style field — the flip is one style op, the
        // readout follows the stored value.
        let mut s = cue_state(Vec::new());
        assert_eq!(s.core.subtitle_style().font, SubtitleFont::Shaped);
        assert_eq!(font_label(&s.core, SubtitleFont::Shaped), "Shaped");
        assert_eq!(font_label(&s.core, SubtitleFont::Pixel), "Pixel");
        assert_eq!(flipped_font(SubtitleFont::Shaped), SubtitleFont::Pixel);
        assert_eq!(flipped_font(SubtitleFont::Pixel), SubtitleFont::Shaped);
        let depth = s.core.undo_depth();
        cycle_font(&mut s);
        assert_eq!(s.core.subtitle_style().font, SubtitleFont::Pixel);
        assert_eq!(s.core.undo_depth(), depth + 1);
        cycle_font(&mut s);
        assert_eq!(s.core.subtitle_style().font, SubtitleFont::Shaped);
        assert_eq!(s.core.undo_depth(), depth + 2);
    }

    #[test]
    fn srt_import_names_dropped_speaker_credits() {
        // RM21 (A9 G9c): the import side of the Wave S
        // export count — replacing a credited list from
        // SRT names the credits the format dropped;
        // WebVTT's import never grows the clause.
        let dir = temp_dir("import-speakers");
        let mut credited = cue(0, 24, "First line");
        credited.speaker = Some("Mary Jane".to_string());
        let mut s = cue_state(vec![credited]);
        let srt = dir.join("plain.srt");
        std::fs::write(&srt, "1\n00:00:02,000 --> 00:00:03,000\nOnly cue\n").unwrap();
        s.subtitle_panel.path = TextEdit::new(srt.to_string_lossy().into_owned());
        import_file(&mut s, SubtitleFileFormat::Srt);
        assert_eq!(
            s.core.status,
            format!(
                "Imported 1 subtitle(s) from SRT — 1 speaker credit(s) dropped (SRT has no speaker syntax)."
            )
        );
        // WebVTT: the same replacement shape, the
        // established plain sentence.
        let mut credited = cue(0, 24, "First line");
        credited.speaker = Some("Mary Jane".to_string());
        let mut s = cue_state(vec![credited]);
        let vtt = dir.join("plain.vtt");
        std::fs::write(
            &vtt,
            "WEBVTT\n\n00:00:02.000 --> 00:00:03.000\nOnly cue\n",
        )
        .unwrap();
        s.subtitle_panel.path = TextEdit::new(vtt.to_string_lossy().into_owned());
        import_file(&mut s, SubtitleFileFormat::Vtt);
        assert_eq!(s.core.status, "Imported 1 subtitle(s) from WebVTT.");
    }

    #[test]
    fn section_constructs_on_every_state_branch() {
        // Element trees are not readable from unit
        // tests — construction is the assertable level
        // here (the design kit's own discipline); the
        // presented-tree proof belongs to the headless
        // snaps.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // No cues: empty state + add + style + file.
            let s = cue_state(Vec::new());
            let _ = subtitle_section(&s, &t);
            // Cues, nothing selected: guidance detail.
            let s = cue_state(vec![cue(0, 24, "a"), cue(24, 48, "b")]);
            let _ = subtitle_section(&s, &t);
            // A live selection: the populated detail.
            let mut s = cue_state(vec![cue(0, 24, "a")]);
            s.subtitle_selected = Some(0);
            let _ = subtitle_section(&s, &t);
            // The editor open on the selection.
            begin_edit(&mut s);
            let _ = subtitle_section(&s, &t);
            // A stale selection tints nothing and breaks
            // nothing.
            let mut s = cue_state(vec![cue(0, 24, "a")]);
            s.subtitle_selected = Some(99);
            let _ = subtitle_section(&s, &t);
        }
    }
}
