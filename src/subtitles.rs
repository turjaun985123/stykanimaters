//! Subtitle subsystem (Wave P, P23): the cue model, the
//! presentation style, the shared stacked-rendering layout
//! maths, and the SRT / WebVTT interchange converters for
//! `Project::subtitles` (board contracts 1–2).
//!
//! The `EditorCore` editing ops over this data (one undo
//! step per mutation, index validated first) live in
//! `editor.rs` beside the core they mutate; everything here
//! is pure data, pure queries over a `&Project`, and pure
//! text conversion, so the renderers (raster burn-in, SVG
//! writer, the Rune stage) and the MCP surfaces all build
//! on the one source.
//!
//! Cue semantics:
//!
//! * A cue is live on the half-open frame range
//!   `[start_frame, end_frame)` — it appears at
//!   `start_frame` and is gone at `end_frame`. Half-open
//!   keeps a one-frame cue valid (`end = start + 1`) and
//!   lets back-to-back cues tile without overlap.
//! * The stored list is kept sorted by
//!   `(start_frame, end_frame)` by the editing ops and by
//!   import callers (via [`sort_subtitle_cues`]); the
//!   converters below return cues in FILE order and never
//!   sort — normalising the order is the caller's job.
//! * Overlapping cues are allowed (stacked lines are a
//!   legitimate authoring choice). `subtitle_at` on the
//!   core returns the first live cue in stored order;
//!   [`live_subtitle_cues`] returns all of them.
//! * Frame numbers are NOT clamped to the timeline: a cue
//!   authored past the last frame simply never shows.
//!   Extending the timeline later can bring it into view.

use serde::{Deserialize, Serialize};

use crate::model::Project;

// ---------------------------------------------------------------------------
// Cue model
// ---------------------------------------------------------------------------

/// One subtitle cue: `text` shown on the half-open frame
/// range `[start_frame, end_frame)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubtitleCue {
    /// First frame the cue is shown on (0-based).
    pub start_frame: usize,
    /// First frame the cue is no longer shown on
    /// (0-based, exclusive). Always `> start_frame` in a
    /// cue the ops accepted.
    pub end_frame: usize,
    /// The cue's text, stored verbatim. Never empty or
    /// all-whitespace in a cue the ops accepted.
    pub text: String,
}

impl SubtitleCue {
    /// The one validation rule every write path shares:
    /// the range must be non-empty (`end_frame >
    /// start_frame`) and the text must contain at least
    /// one non-whitespace character. Returns the refusal
    /// reason in the ops' failure voice (frame numbers
    /// 1-based, as the status line speaks).
    pub fn validate(start_frame: usize, end_frame: usize, text: &str) -> Result<(), String> {
        if end_frame <= start_frame {
            return Err(format!(
                "end frame {} must be after start frame {}",
                end_frame + 1,
                start_frame + 1
            ));
        }
        if text.trim().is_empty() {
            return Err("text is empty".to_string());
        }
        Ok(())
    }

    /// True when this cue is live at `frame`
    /// (`start_frame <= frame < end_frame`).
    pub fn is_live_at(&self, frame: usize) -> bool {
        self.start_frame <= frame && frame < self.end_frame
    }
}

/// The sort key the cue list is kept ordered by.
fn cue_key(cue: &SubtitleCue) -> (usize, usize) {
    (cue.start_frame, cue.end_frame)
}

/// Stable-sort `cues` by `(start_frame, end_frame)`. The
/// normalisation every caller applies after building a
/// cue list outside the editing ops (file imports, MCP
/// handlers): cues with equal ranges keep their relative
/// order.
pub fn sort_subtitle_cues(cues: &mut Vec<SubtitleCue>) {
    cues.sort_by_key(cue_key);
}

/// Every cue in `project` live at `frame`, in the list's
/// kept (stored) order — the renderer-facing query. The
/// raster and SVG writers work from a `&Project` alone
/// (there is no core in the export path), so the query
/// lives here once instead of being restated per writer;
/// `EditorCore::subtitles_at` is this list and
/// `EditorCore::subtitle_at` is its first element.
pub fn live_subtitle_cues(project: &Project, frame: usize) -> Vec<&SubtitleCue> {
    project
        .subtitles
        .iter()
        .filter(|c| c.is_live_at(frame))
        .collect()
}

// ---------------------------------------------------------------------------
// Presentation style
//
// How the cues are presented, as opposed to what they say
// (the cue list above) or when they show (their frame
// ranges). The style is project data: it lives on
// `Project::subtitle_style` with a serde default, so a
// project file saved before this field existed loads with
// the style that reproduces the plain default look —
// bottom-anchored, unscaled, white text over a black
// backing.
//
// Colours use the house colour representation: a plain
// `u32` packed `0xRRGGBB` (the representation
// `Project::background` and `FigureInstance::color`
// carry). There is no colour struct in the codebase to
// reuse. No surface draws a filled backing box — the
// black `box_color` below is the backing/outline colour
// (offset stamps in the raster, stroke under
// `paint-order="stroke"` in SVG), named for the role it
// plays.
// ---------------------------------------------------------------------------

/// Which stage edge the subtitle block is anchored to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubtitlePosition {
    /// Anchored to the bottom edge — the default look.
    #[default]
    Bottom,
    /// Anchored to the top edge.
    Top,
}

/// The smallest font scale a style may carry.
pub const SUBTITLE_FONT_SCALE_MIN: f32 = 0.5;
/// The largest font scale a style may carry.
pub const SUBTITLE_FONT_SCALE_MAX: f32 = 3.0;

/// How subtitle cues are presented: anchor edge, font
/// scale relative to the house subtitle size (see the
/// layout section below), and the text / backing
/// colours.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStyle {
    /// The stage edge the cue block is anchored to.
    pub position: SubtitlePosition,
    /// Multiplier on the house subtitle font size
    /// ([`SUBTITLE_BASE_FONT_SIZE`]). Kept within
    /// [`SUBTITLE_FONT_SCALE_MIN`]..=[`SUBTITLE_FONT_SCALE_MAX`]
    /// by every write path.
    pub font_scale: f32,
    /// Cue text colour, `0xRRGGBB`.
    pub text_color: u32,
    /// Backing colour, `0xRRGGBB`: the outline stroke /
    /// offset stamps that keep the text legible (no
    /// surface draws a filled box; see the section note).
    pub box_color: u32,
}

impl Default for SubtitleStyle {
    /// The plain default look: bottom-anchored, unscaled,
    /// white text (`0xFFFFFF`) over the black backing
    /// (`0x000000`).
    fn default() -> Self {
        SubtitleStyle {
            position: SubtitlePosition::Bottom,
            font_scale: 1.0,
            text_color: 0xFF_FFFF,
            box_color: 0x00_0000,
        }
    }
}

impl SubtitleStyle {
    /// The write-path discipline every setter shares: a
    /// non-finite `font_scale` is REFUSED (returned as an
    /// `Err`, never stored), a finite one is clamped into
    /// [`SUBTITLE_FONT_SCALE_MIN`]..=[`SUBTITLE_FONT_SCALE_MAX`].
    /// Colours are stored verbatim — the house rule for
    /// `0xRRGGBB` values. Returns the style to store.
    pub fn validated(self) -> Result<SubtitleStyle, String> {
        if !self.font_scale.is_finite() {
            return Err(format!(
                "font scale {} is not a finite number",
                self.font_scale
            ));
        }
        Ok(SubtitleStyle {
            font_scale: self
                .font_scale
                .clamp(SUBTITLE_FONT_SCALE_MIN, SUBTITLE_FONT_SCALE_MAX),
            ..self
        })
    }

    /// A read-path sanitised copy, for renderers reading
    /// the stored field directly: the same clamp as
    /// [`SubtitleStyle::validated`], with a non-finite
    /// scale (only reachable from a hand-edited project
    /// file) falling back to 1.0 — the `head_ring_scale`
    /// sanitise-at-read precedent in `model.rs`. Never
    /// mutates the stored style.
    pub fn sanitised(self) -> SubtitleStyle {
        SubtitleStyle {
            font_scale: if self.font_scale.is_finite() {
                self.font_scale
                    .clamp(SUBTITLE_FONT_SCALE_MIN, SUBTITLE_FONT_SCALE_MAX)
            } else {
                1.0
            },
            ..self
        }
    }
}

// ---------------------------------------------------------------------------
// Shared stacked-rendering layout
//
// The band maths every surface that draws live cues
// shares (raster burn-in, SVG text, the stage), so the
// three can never drift apart in where a line sits. The
// constants are the house subtitle metrics in stage
// units, verified against the SVG writer's pins (Wave P,
// P27): at font scale 1.0 a line is 20 units tall on a
// 24-unit step, and the block sits 16 units in from the
// anchored edge.
//
// Stacking: cues stack away from the anchored edge in
// stored order, the FIRST cue closest to the edge, one
// `<text>`-equivalent line band per text line. Within a
// cue at the bottom anchor the cue's LAST line sits
// closest to the edge, so a cue always lands at the same
// position however many lines or cues stack above it.
// ---------------------------------------------------------------------------

/// The house subtitle font size in stage units, before
/// the style's font scale.
pub const SUBTITLE_BASE_FONT_SIZE: f32 = 20.0;
/// The house subtitle line step in stage units, before
/// the style's font scale (1.2× the base font size).
pub const SUBTITLE_BASE_LINE_HEIGHT: f32 = 24.0;
/// Distance in stage units from the anchored stage edge
/// to the nearest line band.
pub const SUBTITLE_EDGE_MARGIN: f32 = 16.0;

/// The font size a renderer draws `style`'s cues at, in
/// stage units: the base size times the style's scale,
/// sanitised at read (a corrupt stored scale can never
/// produce an invisible or absurd subtitle).
pub fn subtitle_font_size(style: SubtitleStyle) -> f32 {
    SUBTITLE_BASE_FONT_SIZE * style.sanitised().font_scale
}

/// The line step for `style`, in stage units: the base
/// line height times the sanitised scale — always
/// 1.2× [`subtitle_font_size`].
pub fn subtitle_line_height(style: SubtitleStyle) -> f32 {
    SUBTITLE_BASE_LINE_HEIGHT * style.sanitised().font_scale
}

/// The distance, in lines, of one cue line from the
/// anchored edge: line `line_index` (0-based, in the
/// cue's own top-to-bottom line order) of a cue with
/// `lines_in_cue` lines, when `lines_before` lines of
/// earlier cues sit between it and the edge.
///
/// * Bottom anchor: within a cue the LAST line is
///   closest to the edge (a cue's lines read downward
///   toward the edge), so the offset counts up as
///   `line_index` counts down.
/// * Top anchor: the cue's lines read away from the
///   edge in their own order.
///
/// A renderer multiplies the offset by its line step and
/// measures from its anchor edge (see
/// [`subtitle_line_center_y`]).
pub fn subtitle_stack_line_offset(
    lines_before: usize,
    lines_in_cue: usize,
    line_index: usize,
    position: SubtitlePosition,
) -> usize {
    match position {
        SubtitlePosition::Bottom => lines_before + (lines_in_cue - 1 - line_index),
        SubtitlePosition::Top => lines_before + line_index,
    }
}

/// The y coordinate (stage space, top-left origin) of
/// the CENTRE of the line band that sits `from_edge`
/// lines away from `style`'s anchored edge, on a stage
/// `stage_height` units tall: the band's block starts
/// [`SUBTITLE_EDGE_MARGIN`] from the edge and each
/// further line steps one line height in. A renderer
/// centring glyphs on this y (SVG's
/// `dominant-baseline="central"`, the raster's centred
/// stamp) reproduces the shared placement exactly.
pub fn subtitle_line_center_y(style: SubtitleStyle, stage_height: f32, from_edge: usize) -> f32 {
    let line_height = subtitle_line_height(style);
    match style.position {
        SubtitlePosition::Bottom => {
            stage_height - SUBTITLE_EDGE_MARGIN - from_edge as f32 * line_height - line_height / 2.0
        }
        SubtitlePosition::Top => {
            SUBTITLE_EDGE_MARGIN + from_edge as f32 * line_height + line_height / 2.0
        }
    }
}

// ---------------------------------------------------------------------------
// SRT import / export
//
// The interchange format for the cue list, spoken by
// external subtitle editors and video tooling. The wire
// shape is the de-facto SRT standard:
//
//     1
//     00:00:00,000 --> 00:00:01,000
//     Cue text, possibly
//     over several lines
//
//     2
//     ...
//
// Cues are numbered from 1, blocks are separated by a
// blank line, and timestamps are `HH:MM:SS,mmm`
// (comma millisecond separator).
//
// Frame <-> time mapping: a cue's half-open frame range
// converts directly — frame `f` begins `f / fps` seconds
// into the timeline, so the SRT end timestamp is the
// moment the cue stops being live, which is exactly
// `end_frame` converted (the end timestamp names the
// boundary, not the last live frame). Going the other
// way, `frame = round(seconds * fps)`; millisecond
// rounding costs at most half a millisecond, far under
// half a frame at any sane fps, so an export -> import
// round trip is frame-exact (pinned by P24's tests).
// ---------------------------------------------------------------------------

/// Format the frame boundary `frame` as an SRT
/// timestamp (`HH:MM:SS,mmm`) at `fps` frames per
/// second.
fn srt_timestamp(frame: usize, fps: f32) -> String {
    let total_ms = (frame as f64 * 1000.0 / fps as f64).round() as u64;
    let ms = total_ms % 1_000;
    let total_s = total_ms / 1_000;
    let s = total_s % 60;
    let m = (total_s / 60) % 60;
    let h = total_s / 3_600;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

/// Parse one SRT timestamp into seconds. Accepts the
/// canonical `HH:MM:SS,mmm` plus the common variants
/// (`.` as the millisecond separator, a missing hours
/// field, 1–3 millisecond digits read as a fraction).
/// Returns `None` on anything else.
fn parse_srt_timestamp(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    let (hours, minutes, sec_part) = match parts.as_slice() {
        [h, m, s] => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, *s),
        [m, s] => (0, m.parse::<u64>().ok()?, *s),
        _ => return None,
    };
    let (sec, ms) = match sec_part.split_once(',').or_else(|| sec_part.split_once('.')) {
        Some((sec, frac)) => {
            if frac.is_empty() || frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let mut ms = frac.parse::<u64>().ok()?;
            for _ in frac.len()..3 {
                ms *= 10;
            }
            (sec.parse::<u64>().ok()?, ms)
        }
        None => (sec_part.parse::<u64>().ok()?, 0),
    };
    Some(hours as f64 * 3600.0 + minutes as f64 * 60.0 + sec as f64 + ms as f64 / 1000.0)
}

/// Convert seconds to the nearest frame at `fps`
/// (`frame = round(seconds * fps)`).
fn srt_seconds_to_frame(seconds: f64, fps: f32) -> usize {
    let frames = (seconds * fps as f64).round();
    if frames <= 0.0 {
        0
    } else {
        frames as usize
    }
}

/// Export the project's cues as SRT text at the
/// project's own frame rate. Cues are written in the
/// list's stored order, numbered from 1, with their
/// text verbatim — multi-line text stays multi-line. A
/// project with no cues exports as the empty string.
pub fn subtitles_to_srt(project: &Project) -> String {
    // Mirror the exporters' fps discipline (export.rs):
    // a non-positive stored rate cannot name frame
    // boundaries, so it falls back to 1 fps rather than
    // dividing by zero. Real projects always carry a
    // sane rate (default 12).
    let fps = project.fps.max(1.0);
    let mut out = String::new();
    for (i, cue) in project.subtitles.iter().enumerate() {
        out.push_str(&(i + 1).to_string());
        out.push('\n');
        out.push_str(&srt_timestamp(cue.start_frame, fps));
        out.push_str(" --> ");
        out.push_str(&srt_timestamp(cue.end_frame, fps));
        out.push('\n');
        out.push_str(&cue.text);
        out.push_str("\n\n");
    }
    out
}

/// Parse one blank-line-separated SRT block into a cue.
/// The error carries the cue's declared index (the
/// number the file gives it, when the block carries
/// one) and the refusal reason.
fn parse_srt_block(lines: &[&str], fps: f32) -> Result<SubtitleCue, (Option<usize>, String)> {
    let declared = lines.first().and_then(|l| l.trim().parse::<usize>().ok());
    let Some(timing_pos) = lines.iter().position(|l| l.contains("-->")) else {
        return Err((declared, "no timing line".to_string()));
    };
    let Some((start_text, end_text)) = lines[timing_pos].split_once("-->") else {
        return Err((declared, "no timing line".to_string()));
    };
    let Some(start_s) = parse_srt_timestamp(start_text) else {
        return Err((declared, format!("bad start timestamp {:?}", start_text.trim())));
    };
    // Anything after the end timestamp on the timing
    // line (position settings in some dialects) is not
    // part of the timestamp.
    let end_token = end_text.split_whitespace().next().unwrap_or("");
    let Some(end_s) = parse_srt_timestamp(end_token) else {
        return Err((declared, format!("bad end timestamp {end_token:?}")));
    };
    let start_frame = srt_seconds_to_frame(start_s, fps);
    // The imported end is clamped to at least start+1:
    // an SRT range that rounds (or is authored) onto a
    // single frame still names a live instant, and the
    // cue model's minimum cue is one frame.
    let end_frame = srt_seconds_to_frame(end_s, fps).max(start_frame.saturating_add(1));
    let text = lines[timing_pos + 1..].join("\n");
    if let Err(reason) = SubtitleCue::validate(start_frame, end_frame, &text) {
        return Err((declared, reason));
    }
    Ok(SubtitleCue {
        start_frame,
        end_frame,
        text,
    })
}

/// Import SRT text as a cue list at `fps` frames per
/// second. The parser is deliberately forgiving, the
/// way subtitle tooling is: CRLF or LF line endings, a
/// leading BOM, unnumbered blocks and the timestamp
/// variants [`parse_srt_timestamp`] accepts are all
/// fine, and any block that cannot be understood — a
/// bad timestamp, no timing line, empty text — is
/// SKIPPED, not fatal. The returned list is in FILE
/// order; sorting is the caller's normalisation
/// ([`sort_subtitle_cues`]).
///
/// The refusals are exactly two: a non-positive or
/// non-finite `fps` (no timestamp can convert), and an
/// import that yields nothing at all — the `Err` then
/// names the first bad cue's index (the number the file
/// gives that cue, or its 1-based block position when
/// unnumbered) and the reason it was refused, so the
/// caller can say what was wrong with the file. Zero
/// yield must refuse: an empty list would silently wipe
/// a project's subtitles on import.
pub fn subtitles_from_srt(text: &str, fps: f32) -> Result<Vec<SubtitleCue>, String> {
    if !fps.is_finite() || fps <= 0.0 {
        return Err(format!("cannot import subtitles at {fps} fps"));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    // Split into blank-line-separated blocks.
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut cues = Vec::new();
    let mut first_bad: Option<(usize, String)> = None;
    for (block_idx, block) in blocks.iter().enumerate() {
        match parse_srt_block(block, fps) {
            Ok(cue) => cues.push(cue),
            Err((declared, reason)) => {
                if first_bad.is_none() {
                    first_bad = Some((declared.unwrap_or(block_idx + 1), reason));
                }
            }
        }
    }
    if !cues.is_empty() {
        return Ok(cues);
    }
    match first_bad {
        Some((index, reason)) => Err(format!(
            "no valid subtitle cues in the SRT text (first bad cue {index}: {reason})"
        )),
        None => Err("no subtitle cues in the SRT text".to_string()),
    }
}

// ---------------------------------------------------------------------------
// WebVTT import / export
//
// The second interchange format for the cue list, the
// one web players and browsers speak. The wire shape
// is the W3C WebVTT standard:
//
//     WEBVTT
//
//     cue-1
//     00:00:00.000 --> 00:00:01.000 align:center
//     Cue text, possibly
//     over several lines
//
//     ...
//
// Against SRT the differences are all at the edges of
// the block grammar, and this module keeps them there:
// the cue blocks themselves parse through the very same
// code as SRT ([`parse_srt_block`],
// [`parse_srt_timestamp`], [`srt_seconds_to_frame`]),
// so the two formats can never drift apart in what a
// timestamp means or what a bad cue costs.
//
// - The file opens with a `WEBVTT` header line (a
//   title may follow the signature on the same line).
//   No header, no import: the refusal is whole-file,
//   before any cue is read.
// - Timestamps are `HH:MM:SS.mmm` — a DOT before the
//   milliseconds where SRT writes a comma. Export is
//   always canonical (hours always present, dot
//   separator). Import runs through the shared SRT
//   timestamp parser, which also accepts the comma
//   separator and a missing hours field; that leniency
//   is deliberate — real-world VTT occasionally carries
//   SRT-flavoured timestamps, and one parser means one
//   meaning.
// - A cue may carry a string identifier line before
//   its timing line (SRT's number is the special case
//   that parses as one). Export writes numeric
//   identifiers, exactly mirroring the SRT export's
//   numbering, so the two exports of one project are
//   block-for-block parallel.
// - Anything after the end timestamp on the timing
//   line is cue settings (`align:`, `position:`, …):
//   tolerated and ignored — this model has nowhere to
//   keep them. (The SRT block parser already discards
//   them the same way.)
// - `NOTE` blocks are comments, and `STYLE` /
//   `REGION` blocks are metadata: all three are
//   skipped outright — never cues, never errors, never
//   the "first bad cue" an empty import complains
//   about.
//
// Frame <-> time mapping is SRT's, unchanged: the
// half-open frame range converts directly at the
// project's fps, so an export -> import round trip is
// frame-exact (pinned by P25's tests).
// ---------------------------------------------------------------------------

/// Format the frame boundary `frame` as a WebVTT
/// timestamp (`HH:MM:SS.mmm`) at `fps` frames per
/// second — [`srt_timestamp`] with the dot separator
/// the format requires.
fn vtt_timestamp(frame: usize, fps: f32) -> String {
    let total_ms = (frame as f64 * 1000.0 / fps as f64).round() as u64;
    let ms = total_ms % 1_000;
    let total_s = total_ms / 1_000;
    let s = total_s % 60;
    let m = (total_s / 60) % 60;
    let h = total_s / 3_600;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// Export the project's cues as WebVTT text at the
/// project's own frame rate: the `WEBVTT` header, then
/// the cues in the list's stored order with numeric
/// identifiers and their text verbatim —
/// block-for-block the SRT export with a header and
/// dot timestamps. A project with no cues exports as
/// the bare header (a valid, empty WebVTT file) rather
/// than SRT's empty string: a WebVTT file without its
/// signature is not a WebVTT file at all.
pub fn subtitles_to_vtt(project: &Project) -> String {
    // Same fps discipline as the SRT export (and the
    // exporters behind it): a non-positive stored rate
    // falls back to 1 fps rather than dividing by zero.
    let fps = project.fps.max(1.0);
    let mut out = String::from("WEBVTT\n\n");
    for (i, cue) in project.subtitles.iter().enumerate() {
        out.push_str(&(i + 1).to_string());
        out.push('\n');
        out.push_str(&vtt_timestamp(cue.start_frame, fps));
        out.push_str(" --> ");
        out.push_str(&vtt_timestamp(cue.end_frame, fps));
        out.push('\n');
        out.push_str(&cue.text);
        out.push_str("\n\n");
    }
    out
}

/// Parse one blank-line-separated WebVTT cue block.
/// Every wire difference from SRT is handled outside
/// the block (the header is stripped before the split,
/// metadata blocks never reach here, and the shared
/// timestamp parser accepts the dot separator), so a
/// cue block is an SRT block verbatim — delegating
/// keeps the two disciplines identical by construction,
/// including the declared-index error reporting (a
/// numeric identifier reports as its number, a string
/// identifier falls back to the block position).
fn parse_vtt_block(lines: &[&str], fps: f32) -> Result<SubtitleCue, (Option<usize>, String)> {
    parse_srt_block(lines, fps)
}

/// Is this block WebVTT metadata rather than a cue?
/// `NOTE` blocks are comments (the word alone on the
/// first line, or followed by text), `STYLE` and
/// `REGION` blocks carry presentation/routing data
/// this model does not keep. Metadata is skipped
/// silently: it is not a cue and it is not an error.
fn vtt_block_is_metadata(lines: &[&str]) -> bool {
    let Some(first) = lines.first() else {
        return false;
    };
    let first = first.trim();
    first == "NOTE" || first.starts_with("NOTE ") || first.starts_with("NOTE\t") || first == "STYLE" || first == "REGION"
}

/// Import WebVTT text as a cue list at `fps` frames
/// per second. The refusal discipline is SRT's,
/// mirrored exactly: the parser is forgiving per cue
/// (CRLF or LF endings, a leading BOM, string or
/// numeric identifiers, cue settings, comma or
/// hour-less timestamps via the shared parser), any
/// cue block that cannot be understood is SKIPPED, and
/// the hard failures are a bad frame rate and an
/// import that yields nothing at all — the `Err` then
/// names the first bad cue, as SRT's does. The
/// returned list is in FILE order; sorting is the
/// caller's normalisation ([`sort_subtitle_cues`]).
///
/// The one whole-file refusal SRT has no analogue for
/// is a missing `WEBVTT` header: without the signature
/// the text is not WebVTT, so it is refused up front
/// rather than mined for cue-shaped blocks.
pub fn subtitles_from_vtt(text: &str, fps: f32) -> Result<Vec<SubtitleCue>, String> {
    if !fps.is_finite() || fps <= 0.0 {
        return Err(format!("cannot import subtitles at {fps} fps"));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    // The header: the first line must be the WEBVTT
    // signature, optionally followed by a title on the
    // same line. Everything after that line is cue
    // territory.
    let mut lines = normalized.split('\n');
    let header = lines.next().unwrap_or("").trim_end();
    let has_header =
        header == "WEBVTT" || header.starts_with("WEBVTT ") || header.starts_with("WEBVTT\t");
    if !has_header {
        return Err("no WEBVTT header in the WebVTT text".to_string());
    }

    // Split the rest into blank-line-separated blocks,
    // exactly as the SRT import does.
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut cues = Vec::new();
    let mut first_bad: Option<(usize, String)> = None;
    for (block_idx, block) in blocks.iter().enumerate() {
        if vtt_block_is_metadata(block) {
            continue;
        }
        match parse_vtt_block(block, fps) {
            Ok(cue) => cues.push(cue),
            Err((declared, reason)) => {
                if first_bad.is_none() {
                    first_bad = Some((declared.unwrap_or(block_idx + 1), reason));
                }
            }
        }
    }
    if !cues.is_empty() {
        return Ok(cues);
    }
    match first_bad {
        Some((index, reason)) => Err(format!(
            "no valid subtitle cues in the WebVTT text (first bad cue {index}: {reason})"
        )),
        None => Err("no subtitle cues in the WebVTT text".to_string()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
        }
    }

    /// A demo project carrying `cues`, timed at 24 fps
    /// (set explicitly — never the demo rate).
    fn project_with(cues: Vec<SubtitleCue>) -> Project {
        let mut project = Project::new_demo();
        project.fps = 24.0;
        project.subtitles = cues;
        project
    }

    #[test]
    fn validate_refuses_inverted_and_empty_ranges() {
        assert!(SubtitleCue::validate(2, 3, "hi").is_ok());
        // One-frame cue: end = start + 1 is the minimum.
        assert!(SubtitleCue::validate(2, 3, "x").is_ok());
        assert!(SubtitleCue::validate(2, 2, "hi").is_err());
        assert!(SubtitleCue::validate(3, 2, "hi").is_err());
        assert!(SubtitleCue::validate(0, 1, "").is_err());
        assert!(SubtitleCue::validate(0, 1, "   \n ").is_err());
    }

    #[test]
    fn sort_is_by_start_then_end_and_stable() {
        let mut cues = vec![
            cue(24, 48, "later"),
            cue(0, 24, "first"),
            cue(0, 12, "same start, earlier end"),
            cue(0, 24, "first two"),
        ];
        sort_subtitle_cues(&mut cues);
        let texts: Vec<&str> = cues.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            ["same start, earlier end", "first", "first two", "later"],
            "sorted by (start, end); equal keys keep their relative order"
        );
    }

    #[test]
    fn live_cues_use_the_half_open_range_in_stored_order() {
        let project = project_with(vec![cue(2, 5, "a"), cue(4, 8, "b"), cue(9, 10, "c")]);
        let texts = |frame: usize| -> Vec<&str> {
            live_subtitle_cues(&project, frame)
                .iter()
                .map(|c| c.text.as_str())
                .collect()
        };
        assert!(texts(1).is_empty());
        assert_eq!(texts(2), ["a"]);
        assert_eq!(texts(4), ["a", "b"], "overlaps all report, stored order");
        assert_eq!(texts(5), ["b"], "end_frame is exclusive");
        assert_eq!(texts(9), ["c"]);
        assert!(texts(10).is_empty());
    }

    #[test]
    fn default_style_is_the_plain_look() {
        let style = SubtitleStyle::default();
        assert_eq!(style.position, SubtitlePosition::Bottom);
        assert_eq!(style.font_scale, 1.0);
        assert_eq!(style.text_color, 0xFF_FFFF);
        assert_eq!(style.box_color, 0x00_0000);
    }

    #[test]
    fn style_validated_clamps_and_refuses_non_finite() {
        let clamped = SubtitleStyle {
            font_scale: 9.0,
            ..SubtitleStyle::default()
        }
        .validated()
        .expect("a finite scale validates");
        assert_eq!(clamped.font_scale, SUBTITLE_FONT_SCALE_MAX);
        assert!(SubtitleStyle {
            font_scale: f32::NAN,
            ..SubtitleStyle::default()
        }
        .validated()
        .is_err());
        assert!(SubtitleStyle {
            font_scale: f32::INFINITY,
            ..SubtitleStyle::default()
        }
        .validated()
        .is_err());
    }

    #[test]
    fn style_sanitised_falls_back_to_unscaled() {
        let corrupt = SubtitleStyle {
            font_scale: f32::NAN,
            ..SubtitleStyle::default()
        };
        assert_eq!(corrupt.sanitised().font_scale, 1.0);
        // The stored value is never mutated by the read.
        assert!(corrupt.font_scale.is_nan());
    }

    #[test]
    fn layout_matches_the_svg_writers_verified_bands() {
        // P27's pins, reproduced against the shared
        // helper: stage 800x600, default style.
        let style = SubtitleStyle::default();
        assert_eq!(subtitle_font_size(style), 20.0);
        assert_eq!(subtitle_line_height(style), 24.0);
        assert_eq!(subtitle_line_center_y(style, 600.0, 0), 572.0, "Bottom band");
        let top = SubtitleStyle {
            position: SubtitlePosition::Top,
            ..SubtitleStyle::default()
        };
        assert_eq!(subtitle_line_center_y(top, 600.0, 0), 28.0, "Top band");
        // Two-cue stack: the second cue's only line sits
        // one line further from the edge.
        let off = subtitle_stack_line_offset(1, 1, 0, style.position);
        assert_eq!(off, 1);
        assert_eq!(subtitle_line_center_y(style, 600.0, off), 548.0);
        assert_eq!(subtitle_line_center_y(top, 600.0, off), 52.0);
        // Scale 2.0: font 40, Bottom band 560.
        let big = SubtitleStyle {
            font_scale: 2.0,
            ..SubtitleStyle::default()
        };
        assert_eq!(subtitle_font_size(big), 40.0);
        assert_eq!(subtitle_line_center_y(big, 600.0, 0), 560.0);
    }

    #[test]
    fn stack_offset_counts_down_within_a_bottom_cue() {
        // A two-line cue at the bottom: its LAST line is
        // offset 0 (closest to the edge), its first line
        // offset 1; at the top the order is its own.
        assert_eq!(subtitle_stack_line_offset(0, 2, 0, SubtitlePosition::Bottom), 1);
        assert_eq!(subtitle_stack_line_offset(0, 2, 1, SubtitlePosition::Bottom), 0);
        assert_eq!(subtitle_stack_line_offset(0, 2, 0, SubtitlePosition::Top), 0);
        assert_eq!(subtitle_stack_line_offset(0, 2, 1, SubtitlePosition::Top), 1);
        // Lines of earlier cues shift every later line.
        assert_eq!(subtitle_stack_line_offset(3, 2, 1, SubtitlePosition::Bottom), 3);
        assert_eq!(subtitle_stack_line_offset(3, 2, 0, SubtitlePosition::Top), 3);
    }

    #[test]
    fn srt_round_trip_is_frame_exact_at_24_fps() {
        let cues = vec![
            cue(0, 24, "The gate is open."),
            cue(7, 30, "Off grid, on purpose."),
            cue(48, 96, "First line\nSecond line"),
        ];
        let project = project_with(cues.clone());
        let srt = subtitles_to_srt(&project);
        assert!(
            srt.contains("00:00:00,292 --> 00:00:01,250"),
            "frame 7 exports as 292 ms at 24 fps: {srt:?}"
        );
        let imported = subtitles_from_srt(&srt, 24.0).expect("own export re-imports");
        assert_eq!(imported, cues);
    }

    #[test]
    fn srt_import_skips_bad_blocks_and_refuses_zero_yield() {
        let srt = "1\n00:00:00,000 --> 00:00:01,000\nGood one\n\n\
                   2\nthis block is broken\n\n\
                   3\n00:00:02,000 --> 00:00:03,500\nGood two\n";
        let imported = subtitles_from_srt(srt, 24.0).expect("a partial file imports");
        assert_eq!(imported, vec![cue(0, 24, "Good one"), cue(48, 84, "Good two")]);
        assert!(subtitles_from_srt("not a subtitle file at all\n", 24.0).is_err());
        assert!(subtitles_from_srt("", 24.0).is_err(), "empty text is zero yield");
    }

    #[test]
    fn import_refuses_a_non_positive_fps() {
        let srt = subtitles_to_srt(&project_with(vec![cue(0, 24, "Timed.")]));
        assert!(subtitles_from_srt(&srt, 0.0).is_err());
        assert!(subtitles_from_srt(&srt, -24.0).is_err());
        assert!(subtitles_from_srt(&srt, f32::NAN).is_err());
        assert!(subtitles_from_vtt("WEBVTT\n", 0.0).is_err());
    }

    #[test]
    fn empty_project_exports_empty_srt_and_bare_vtt_header() {
        let project = project_with(Vec::new());
        assert_eq!(subtitles_to_srt(&project), "");
        assert_eq!(subtitles_to_vtt(&project), "WEBVTT\n\n");
        assert!(subtitles_from_vtt(&subtitles_to_vtt(&project), 24.0).is_err());
    }

    #[test]
    fn vtt_round_trip_and_header_discipline() {
        let cues = vec![cue(0, 24, "First cue"), cue(7, 30, "Off-grid cue")];
        let project = project_with(cues.clone());
        let vtt = subtitles_to_vtt(&project);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:00.292 --> 00:00:01.250"));
        let imported = subtitles_from_vtt(&vtt, 24.0).expect("own export re-imports");
        assert_eq!(imported, cues);
        // Headerless text refuses whole-file even when
        // its cues would parse.
        subtitles_from_vtt("00:00:00.000 --> 00:00:01.000\nFirst cue\n", 24.0)
            .expect_err("no header, no import");
        subtitles_from_vtt("", 24.0).expect_err("empty text has no header");
    }

    #[test]
    fn vtt_metadata_blocks_are_skipped_not_counted() {
        let text = "WEBVTT\n\nSTYLE\n::cue {\n  color: lime;\n}\n\n\
                    NOTE a comment\nspanning lines\n\n\
                    1\n00:00:01.000 --> 00:00:03.000\nStyled cue\n";
        let imported = subtitles_from_vtt(text, 24.0).expect("metadata is tolerated");
        assert_eq!(imported, vec![cue(24, 72, "Styled cue")]);
        // Metadata alone is zero yield, not success.
        subtitles_from_vtt("WEBVTT\n\nSTYLE\n::cue { color: lime; }\n", 24.0)
            .expect_err("metadata does not count as yield");
    }

    #[test]
    fn cue_and_style_round_trip_through_serde() {
        let mut project = project_with(vec![cue(8, 12, "second"), cue(0, 8, "first")]);
        project.subtitle_style = SubtitleStyle {
            position: SubtitlePosition::Top,
            font_scale: 1.5,
            text_color: 0x12_34_56,
            box_color: 0x65_43_21,
        };
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.subtitles, project.subtitles);
        assert_eq!(loaded.subtitle_style, project.subtitle_style);
        // A legacy document — the same project with the
        // three Wave P keys absent, as every file saved
        // before contracts 1–2 wrote it — loads with the
        // defaults (the contract-1/2 serde story).
        let mut legacy_json: serde_json::Value = serde_json::from_str(&json).unwrap();
        let object = legacy_json.as_object_mut().unwrap();
        object.remove("subtitles");
        object.remove("subtitle_style");
        object.remove("format_version");
        let legacy: Project =
            serde_json::from_value(legacy_json).expect("a legacy document loads");
        assert!(legacy.subtitles.is_empty());
        assert_eq!(legacy.subtitle_style, SubtitleStyle::default());
        assert_eq!(legacy.format_version, crate::model::FORMAT_VERSION_CURRENT);
    }
}
