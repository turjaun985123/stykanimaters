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
//! * The cue text field is the kit's one field
//!   construction, which is single-line: a multi-line
//!   cue's line breaks ride in the buffer verbatim and
//!   survive an unedited Apply untouched (the unchanged
//!   check), but the field itself edits the text as one
//!   line, like every field in this shell.
//! * Cue frames are NOT clamped to the timeline (the
//!   core ops' rule): a cue may start or end past the
//!   last frame, and the end stepper has no upper bound.
//! * Drag-to-move for cues is the lane's named remainder
//!   (P32): no per-block drag machinery exists in the
//!   timeline to extend, and this panel does not fake
//!   one — range steppers and the playhead buttons are
//!   the move controls.
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
//! `subtitle-file` (file root), `subtitle-path` (path
//! field), `subtitle-export-srt` / `subtitle-export-vtt`,
//! `subtitle-import-srt` / `subtitle-import-vtt`.
//! No existing key is renamed or removed.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El, TextEdit};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::subtitles::{
    sort_subtitle_cues, subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt,
    subtitles_to_srt_counted, subtitles_to_vtt, SubtitleCue, SubtitlePosition, SubtitleStyle,
    SUBTITLE_FONT_SCALE_MAX, SUBTITLE_FONT_SCALE_MIN,
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
/// below carries the text whole.
pub fn cue_row_label(cue: &SubtitleCue) -> String {
    // A credited cue's row carries its speaker as the
    // `Name: ` prefix — the same composition the
    // renderers draw (subtitles::subtitle_display_lines),
    // panel-local per IA-R21-5.
    let text = match &cue.speaker {
        Some(name) => format!("{name}: {}", cue.text.replace('\n', " ")),
        None => cue.text.replace('\n', " "),
    };
    format!("{}–{} · {}", cue.start_frame + 1, cue.end_frame, text)
}

/// The detail block's range line: the same window as
/// [`cue_row_label`], with the cue's length spelled
/// out. The two words resolve through the i18n tables
/// (`frames` + `subtitle_frames_unit`); the numbers
/// and the window punctuation are locale-invariant.
pub fn cue_range_line(core: &EditorCore, cue: &SubtitleCue) -> String {
    format!(
        "{} {}–{} · {} {}",
        i18n_wire::tr(core, "frames"),
        cue.start_frame + 1,
        cue.end_frame,
        cue.end_frame - cue.start_frame,
        i18n_wire::tr(core, "subtitle_frames_unit")
    )
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
/// at tenth precision, the stepping grain.
pub fn scale_readout(style: SubtitleStyle) -> String {
    format!("{:.1}×", style.sanitised().font_scale)
}

/// A stored colour as the readout's `#RRGGBB`.
pub fn hex_label(rgb: u32) -> String {
    format!("#{rgb:06X}")
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
    let count = cues.len();
    s.core.snapshot();
    s.core.project.subtitles = cues;
    s.subtitle_selected = None;
    close_edit(s);
    s.core.status = format!("Imported {count} subtitle(s) from {label}.");
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
        section = section.child(design::empty_state(
            "star",
            &i18n_wire::tr(core, "subtitle_none"),
            t,
        ));
    } else {
        for (i, cue) in cues.iter().enumerate() {
            let selected = s.subtitle_selected == Some(i);
            section = section.child(
                design::list_row(&format!("subtitle-row-{i}"), selected, t)
                    .w_full()
                    .on_press(move |s: &mut State| select_cue(s, i))
                    .child(
                        text(cue_row_label(cue))
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
    }
    section = section.child(style_block(s, t));
    section = section.child(file_block(s, t));
    section
}

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
            text(format!(
                "{} {}",
                i18n_wire::tr(&s.core, "subtitle_cue"),
                i + 1
            ))
                .font(type_::TITLE)
                .color(t.text),
        )
        .child(
            text(cue_range_line(&s.core, &cue))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    if edit_is_live(s) {
        // The open text session: the speaker field over
        // the cue-text field (Enter applies in either,
        // like every committing field in the shell) over
        // the Apply/Cancel row.
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
            .child(
                design::text_field(
                    &s.subtitle_panel.text,
                    type_::BODY,
                    s.subtitle_panel.text.focused,
                    t,
                )
                .key("subtitle-text-field")
                .single_line()
                .w_full()
                .color(t.text)
                .on_edit(|s: &mut State| &mut s.subtitle_panel.text)
                .on_submit(|s: &mut State, _tx: &mut rune::Tx| apply_edit(s)),
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
fn style_block(s: &State, t: &Theme) -> El {
    let style = s.core.subtitle_style();
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
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    design::section_label(&i18n_wire::tr(&s.core, "subtitle_font_scale"), t)
                        .grow(1.0),
                )
                .child(btn("subtitle-scale-dn", "−", false, t, |s| step_scale(s, -1)))
                .child(design::stepper_value(&scale_readout(style), t))
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
        .child(
            text(format!(
                "{} {} · {} {}",
                i18n_wire::tr(&s.core, "subtitle_text"),
                hex_label(style.text_color),
                i18n_wire::tr(&s.core, "subtitle_box"),
                hex_label(style.box_color)
            ))
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
        let st = SubtitlePanelState::default();
        assert_eq!(st.text_target, None);
    }

    #[test]
    fn labels_speak_the_ops_voice() {
        assert_eq!(cue_row_label(&cue(0, 24, "Hello.")), "1–24 · Hello.");
        assert_eq!(
            cue_row_label(&cue(4, 6, "one\ntwo")),
            "5–6 · one two",
            "line breaks flatten for the one-line row"
        );
        let s = cue_state(Vec::new());
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
        let style = SubtitleStyle::default();
        assert_eq!(stepped_scale(style, 1), 1.1);
        assert_eq!(stepped_scale(style, -1), 0.9);
        assert_eq!(scale_readout(style), "1.0×");
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
        assert_eq!(scale_readout(hi), "3.0×");
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
        const LABELS: [(&str, &str); 31] = [
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
        ];
        let s = cue_state(Vec::new());
        for (key, english) in LABELS {
            assert!(
                i18n_wire::key_in_table(key),
                "{key} missing from the catalogue"
            );
            assert_eq!(i18n_wire::tr(&s.core, key), english, "{key} in English");
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
