//! MCP subtitle cue and style tools (Wave P, P29): the
//! seven editing tools over the subtitle core in
//! [`crate::subtitles`] — `subtitle_list`, `subtitle_add`,
//! `subtitle_update`, `subtitle_delete`,
//! `subtitle_set_style`, `subtitle_shift_all` and
//! `subtitle_retime`. (The SRT/WebVTT file exchange
//! lives in the sibling module, `mcp_subtitle_io`, P30.)
//!
//! The discipline is the core ops' own: they own
//! everything — validation (non-empty half-open range,
//! non-whitespace text, the cue exists, the style's font
//! scale finite), the sorted insert / re-sort, and the
//! undo bookkeeping. Each successful op snapshots exactly
//! once, so these handlers never call `snapshot()`
//! themselves (a second snapshot here would make every
//! mutation two undo steps). A refused op changes
//! nothing, takes no snapshot and explains itself on the
//! core's status line; the handlers surface that line as
//! the error. All arguments are read and validated BEFORE
//! the op runs, so a malformed argument never leaves a
//! half-applied change behind — and the ops validate
//! again before they snapshot, so index/range/style
//! refusals always land before any undo step is spent.
//!
//! Indices and frames are 0-based, the convention of
//! every index in this registry; the ops' status lines
//! report 1-based, as the GUI does. `subtitle_list` is
//! read-only: no snapshot, no status-line write.
//! `subtitle_set_style` is a full replace (all four
//! arguments required) and its reply's `style` is the
//! style as stored, after the op's clamping.
//!
//! No-change parity (Wave R, R20): the Subtitles panel
//! never spends an undo step on a press that would change
//! nothing — its colour swatches, scale stepper and text
//! apply all guard before calling the ops, which snapshot
//! unconditionally. These handlers keep the same rule on
//! this surface: a `subtitle_update` identical to the cue
//! as stored, and a `subtitle_set_style` whose clamped
//! result equals the style in force, reply `changed:
//! false` and spend no step. And because the panel's
//! one-field-at-a-time style gestures are read-modify-write
//! over `EditorCore::subtitle_style`, `subtitle_list`
//! carries that same read: its reply's `style` is the
//! style in force, so a client can compose a faithful
//! partial change through the full-replace tool.
//!
//! Speakers (Wave R, R21): a cue may carry a speaker
//! name — metadata, normalised by the core
//! (`subtitles::normalise_speaker`: trimmed, empty
//! stores none, tag-breaking names refused by the op).
//! `subtitle_add` and `subtitle_update` accept it as an
//! optional `speaker` argument; because `subtitle_update`
//! is a full replace, an absent or empty argument CLEARS
//! the cue's speaker — `subtitle_list` reports each
//! cue's speaker (the field is present only when the
//! cue carries one) so a client can compose a faithful
//! replace. The no-change rule compares the speaker too:
//! a speaker-only difference is a real change and spends
//! its one step.
//!
//! Bulk timing (Roadmap Wave 1, RM21): `subtitle_shift_all`
//! and `subtitle_retime` are the MCP parity of the panel's
//! Timing block, over the core ops
//! `EditorCore::subtitle_shift_all` /
//! `EditorCore::subtitle_retime`, which own the bulk
//! discipline (clone, transform, re-sort, exactly one
//! snapshot on a real change). A call that moves nothing
//! reports the op's status line with a zero count and
//! spends no undo step — the handlers never snapshot
//! themselves, like every handler in this module.

use crate::editor::EditorCore;
use crate::mcp::Tool;
use crate::subtitles::{
    normalise_speaker, SubtitleCue, SubtitleFont, SubtitlePosition, SubtitleStyle,
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------
// Argument helpers (the `mcp_tools` shapes, duplicated the
// way this tree's MCP modules duplicate their small
// helpers — same refusal sentences, including the Wave 11
// finiteness rule: an overflowing JSON literal like 1e999
// can surface as f64 infinity where the parser hands it
// through, and a finite f64 like 1e300 still casts to f32
// infinity; both must be refused).
// ---------------------------------------------------------------------

fn sub_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn sub_finite_f32(f: f64, key: &str) -> Result<f32, String> {
    let v = f as f32;
    if f.is_finite() && v.is_finite() {
        Ok(v)
    } else {
        Err(format!("argument '{key}' must be a finite number"))
    }
}

fn sub_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match sub_get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => sub_finite_f32(f, key),
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn sub_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn sub_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match sub_get(args, key) {
        Some(v) => sub_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn sub_req_i32(args: &Value, key: &str) -> Result<i32, String> {
    match sub_get(args, key) {
        Some(v) => {
            if let Some(i) = v.as_i64() {
                return i32::try_from(i)
                    .map_err(|_| format!("argument '{key}' is too large"));
            }
            if let Some(f) = v.as_f64() {
                if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                    return Ok(f as i32);
                }
            }
            Err(format!("argument '{key}' must be an integer"))
        }
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn sub_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match sub_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// An optional string argument: absent (or null) is
/// `None`; a present non-string refuses in the required
/// helper's voice.
fn sub_opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match sub_get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

/// Colours travel as `0xRRGGBB` integers.
fn sub_opt_color(args: &Value, key: &str) -> Result<Option<u32>, String> {
    match sub_get(args, key) {
        Some(v) => {
            let n = sub_as_usize(v, key)?;
            u32::try_from(n)
                .map(Some)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------
// Reply shapes
// ---------------------------------------------------------------------

/// The project summary the mutation replies carry (the
/// `mcp_tools::project_summary` shape, duplicated per the
/// module idiom).
fn sub_project_summary(core: &EditorCore) -> Value {
    json!({
        "name": core.project.name,
        "fps": core.project.fps,
        "stage": core.project.stage,
        "figures": core.project.figures.len(),
        "frames": core.project.frames.len(),
        "current_frame": core.current,
    })
}

/// One cue as JSON: its index in the kept (sorted) list,
/// its half-open frame range and its text — the shape
/// `subtitle_list` reports and the mutation tools echo.
/// The cue's speaker rides along as `speaker` ONLY when
/// it carries one (Wave R, R21): an additive field,
/// absent for uncredited cues, so a consumer written
/// before the field existed reads the same shape it
/// always did.
fn subtitle_cue_json(index: usize, cue: &SubtitleCue) -> Value {
    let mut value = json!({
        "index": index,
        "start_frame": cue.start_frame,
        "end_frame": cue.end_frame,
        "text": cue.text,
    });
    if let Some(speaker) = &cue.speaker {
        value["speaker"] = json!(speaker);
    }
    value
}

/// One style as JSON — the shape `subtitle_set_style`
/// echoes back: the style as stored, after the op's
/// clamping.
fn subtitle_style_json(style: SubtitleStyle) -> Value {
    json!({
        "position": match style.position {
            SubtitlePosition::Bottom => "bottom",
            SubtitlePosition::Top => "top",
        },
        "font_scale": style.font_scale,
        "text_color": style.text_color,
        "box_color": style.box_color,
        "font": match style.font {
            SubtitleFont::Shaped => "shaped",
            SubtitleFont::Pixel => "pixel",
        },
    })
}

/// Parse a subtitle position name for
/// `subtitle_set_style`: the two anchor edges, with the
/// same error wording as `mcp_tools`' `parse_interp_name`.
fn parse_subtitle_position(name: &str) -> Result<SubtitlePosition, String> {
    match name {
        "bottom" => Ok(SubtitlePosition::Bottom),
        "top" => Ok(SubtitlePosition::Top),
        other => Err(format!(
            "unknown subtitle position '{other}' (expected \"bottom\" or \"top\")"
        )),
    }
}

/// Parse a subtitle font name (Wave RM, RM2): the raster
/// renderer choice — real shaped text or the legacy 3×5
/// pixel stamp.
fn parse_subtitle_font(name: &str) -> Result<SubtitleFont, String> {
    match name {
        "shaped" => Ok(SubtitleFont::Shaped),
        "pixel" => Ok(SubtitleFont::Pixel),
        other => Err(format!(
            "unknown subtitle font '{other}' (expected \"shaped\" or \"pixel\")"
        )),
    }
}

// ---------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------

fn tool_subtitle_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let subtitles: Vec<Value> = core
        .project
        .subtitles
        .iter()
        .enumerate()
        .map(|(i, cue)| subtitle_cue_json(i, cue))
        .collect();
    Ok(json!({
        "ok": true,
        "subtitles": subtitles,
        "count": core.project.subtitles.len(),
        // The style in force, in the shape
        // `subtitle_set_style` echoes: the read half of
        // the panel's read-modify-write style gestures
        // (Wave R, R20).
        "style": subtitle_style_json(core.project.subtitle_style),
    }))
}

fn tool_subtitle_add(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let start_frame = sub_req_usize(args, "start_frame")?;
    let end_frame = sub_req_usize(args, "end_frame")?;
    let text = sub_req_str(args, "text")?;
    // The optional speaker rides to the op raw: the op
    // normalises it (and refuses a tag-breaking name in
    // its own voice), so there is exactly one refusal
    // surface for it.
    let speaker = sub_opt_str(args, "speaker")?;
    match core.subtitle_add(start_frame, end_frame, text, speaker) {
        Some(index) => Ok(json!({
            "ok": true,
            "index": index,
            "status": core.status,
            "project": sub_project_summary(core),
        })),
        // The op refused (inverted/empty range or empty
        // text) and explained via the status line.
        None => Err(core.status.clone()),
    }
}

fn tool_subtitle_update(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = sub_req_usize(args, "index")?;
    let start_frame = sub_req_usize(args, "start_frame")?;
    let end_frame = sub_req_usize(args, "end_frame")?;
    let text = sub_req_str(args, "text")?;
    let speaker = sub_opt_str(args, "speaker")?;
    // No-change parity (Wave R, R20): an update identical
    // to the cue as stored would re-sort a list that
    // cannot move — the stored list is the same either
    // way — so, like the panel's unchanged text apply,
    // it reports and spends no undo step. A missing index
    // falls through to the op, whose refusal voice is the
    // one this surface already speaks.
    //
    // The comparison covers the speaker too (Wave R,
    // R21): the replace is full, so the would-be stored
    // speaker is the argument normalised — an absent or
    // empty argument means NO speaker, and a
    // speaker-only difference is a real change. A name
    // the normaliser refuses can never equal a stored
    // speaker, so it skips the guard and falls through
    // to the op, which refuses it in its own voice.
    if let Ok(want_speaker) = normalise_speaker(speaker) {
        if let Some(cue) = core.project.subtitles.get(index) {
            if cue.start_frame == start_frame
                && cue.end_frame == end_frame
                && cue.text == text
                && cue.speaker.as_deref() == want_speaker.as_deref()
            {
                core.status =
                    "Subtitle update matched the cue as stored; nothing changed.".to_string();
                return Ok(json!({
                    "ok": true,
                    "changed": false,
                    "index": index,
                    "status": core.status,
                    "project": sub_project_summary(core),
                }));
            }
        }
    }
    match core.subtitle_update(index, start_frame, end_frame, text, speaker) {
        // The op re-sorts after the replace: the reply's
        // index is the cue's position in the list now,
        // which may differ from the index addressed.
        Some(new_index) => Ok(json!({
            "ok": true,
            "changed": true,
            "index": new_index,
            "status": core.status,
            "project": sub_project_summary(core),
        })),
        // The op refused (no such cue, inverted/empty
        // range or empty text) and explained via the
        // status line.
        None => Err(core.status.clone()),
    }
}

fn tool_subtitle_delete(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = sub_req_usize(args, "index")?;
    if !core.subtitle_delete(index) {
        // The op refused (no such cue) and explained via
        // the status line.
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "index": index,
        "status": core.status,
        "project": sub_project_summary(core),
    }))
}

fn tool_subtitle_set_style(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // All arguments are read before the op runs (the
    // module discipline), so a malformed argument never
    // leaves a half-applied change behind. `sub_req_f32`
    // refuses a non-finite scale at parse; the op clamps
    // a finite out-of-range one.
    let position = parse_subtitle_position(sub_req_str(args, "position")?)?;
    let font_scale = sub_req_f32(args, "font_scale")?;
    let text_color = sub_opt_color(args, "text_color")?
        .ok_or_else(|| "missing required argument 'text_color'".to_string())?;
    let box_color = sub_opt_color(args, "box_color")?
        .ok_or_else(|| "missing required argument 'box_color'".to_string())?;
    // Wave RM (RM2): the font is the one OPTIONAL argument
    // — absent means "keep the font in force", so a caller
    // written against the four-argument form can never
    // silently reset a user's deliberate pixel-style choice.
    let font = match sub_opt_str(args, "font")? {
        Some(name) => parse_subtitle_font(name)?,
        None => core.subtitle_style().font,
    };
    let style = SubtitleStyle {
        position,
        font_scale,
        text_color,
        box_color,
        font,
    };
    // No-change parity (Wave R, R20): the op stores
    // `validated()`; for the finite scale the parse above
    // guarantees, that stored form is `sanitised()`
    // exactly (the same clamp), so the would-be result
    // can be compared against the style in force before
    // the op runs. An equal result is a no-op — the
    // panel's swatches and stepper guard the same way —
    // and spends no undo step. A stored style that is
    // itself unsanitised (a hand-edited file) never
    // compares equal here, so the op still runs and the
    // rewrite lands as the real change it is.
    if style.sanitised() == core.subtitle_style() {
        core.status = "Subtitle style matched the current style; nothing changed.".to_string();
        return Ok(json!({
            "ok": true,
            "changed": false,
            "style": subtitle_style_json(core.project.subtitle_style),
            "status": core.status,
            "project": sub_project_summary(core),
        }));
    }
    // The op validates, snapshots exactly once on success
    // (one undo step) and explains a refusal on the status
    // line.
    core.set_subtitle_style(style)
        .map_err(|_| core.status.clone())?;
    Ok(json!({
        "ok": true,
        "changed": true,
        "style": subtitle_style_json(core.project.subtitle_style),
        "status": core.status,
        "project": sub_project_summary(core),
    }))
}

fn tool_subtitle_shift_all(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let ms = sub_req_i32(args, "ms")?;
    // The op owns the whole discipline (conversion,
    // clamping, re-sort, the one snapshot on a real
    // shift, the status line in both outcomes).
    let moved = core.subtitle_shift_all(ms);
    Ok(json!({
        "ok": true,
        "moved": moved,
        "count": core.subtitle_count(),
        "status": core.status,
        "project": sub_project_summary(core),
    }))
}

fn tool_subtitle_retime(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let from_fps = sub_req_f32(args, "from_fps")?;
    if from_fps <= 0.0 {
        return Err("argument 'from_fps' must be greater than 0".to_string());
    }
    let retimed = core.subtitle_retime(from_fps);
    Ok(json!({
        "ok": true,
        "retimed": retimed,
        "count": core.subtitle_count(),
        "status": core.status,
        "project": sub_project_summary(core),
    }))
}

// ---------------------------------------------------------------------
// registration — wired into `mcp::all_tools()` since
// Wave Q (Q1): the lib.rs module line and the all_tools
// extend are in `src/mcp.rs`, after the io family.
// ---------------------------------------------------------------------

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "subtitle_list",
            description: "List the project's subtitle cues in their kept order (sorted by start frame, then end frame): each cue's index in the list, its half-open frame range [start_frame, end_frame) and its text. A cue's speaker is reported as 'speaker' when the cue carries one; the field is absent when it does not. Indices are 0-based, the convention of every index in this registry. The reply also carries the project's subtitle style in 'style' — the style in force, in the shape subtitle_set_style echoes — so a caller can compose a one-field style change from true values. Read-only. (Wave P, P29; style read added Wave R, R20; speaker report Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            handler: tool_subtitle_list,
        },
        Tool {
            name: "subtitle_add",
            description: "Add a subtitle cue over the half-open frame range [start_frame, end_frame) with the given text: the cue shows at start_frame and is gone at end_frame, so end_frame must be after start_frame and the text must not be empty. An optional 'speaker' names who speaks the line: it is trimmed, an empty or whitespace-only name stores no speaker, and a name containing '<', '>' or a newline is refused. The cue list is kept sorted by (start_frame, end_frame) and the reply's 'index' is the new cue's position in it. Frames are 0-based and are not clamped to the timeline. Refused cues (bad range, empty text, unusable speaker name) change nothing. One undo step. (Wave P, P29; speaker Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "start_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is shown on (0-based)" },
                    "end_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is no longer shown on (0-based, exclusive; must be after start_frame)" },
                    "text": { "type": "string", "description": "The cue's text (must not be empty)" },
                    "speaker": { "type": "string", "description": "Optional speaker name for the cue; trimmed, an empty name stores no speaker" }
                },
                "required": ["start_frame", "end_frame", "text"]
            }),
            handler: tool_subtitle_add,
        },
        Tool {
            name: "subtitle_update",
            description: "Replace the subtitle cue at 'index' (its index in subtitle_list) with a new half-open frame range [start_frame, end_frame) and text — a full replace, all four arguments required. The optional 'speaker' is part of the replace: the cue's speaker becomes the given name (trimmed; an empty or whitespace-only name, or omitting the argument, clears it), and a name containing '<', '>' or a newline is refused — read the cue with subtitle_list first to compose a replace that keeps its speaker. The list is re-sorted afterwards and the reply's 'index' is the cue's position in the list now, which may differ from the index addressed. Refused updates (no such cue, bad range, empty text, unusable speaker name) change nothing. An update identical to the cue as stored is a no-op and spends no undo step; the reply's 'changed' says whether anything moved. One undo step. (Wave P, P29; no-op rule Wave R, R20; speaker Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "minimum": 0, "description": "Index of the cue in subtitle_list" },
                    "start_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is shown on (0-based)" },
                    "end_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is no longer shown on (0-based, exclusive; must be after start_frame)" },
                    "text": { "type": "string", "description": "The cue's text (must not be empty)" },
                    "speaker": { "type": "string", "description": "Optional speaker name for the cue; trimmed, an empty name (or omitting it) clears the speaker" }
                },
                "required": ["index", "start_frame", "end_frame", "text"]
            }),
            handler: tool_subtitle_update,
        },
        Tool {
            name: "subtitle_delete",
            description: "Delete the subtitle cue at 'index' (its index in subtitle_list). Refused (no such cue) changes nothing. One undo step. (Wave P, P29)",
            schema: json!({
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "minimum": 0, "description": "Index of the cue in subtitle_list" }
                },
                "required": ["index"]
            }),
            handler: tool_subtitle_delete,
        },
        Tool {
            name: "subtitle_set_style",
            description: "Replace the project's subtitle style in one call — a full replace: 'position' (\"bottom\" or \"top\", the stage edge the cue block is anchored to), 'font_scale' (multiplier on the renderers' house subtitle size; a finite value is clamped to 0.5-3.0, a non-finite one is refused), 'text_color' and 'box_color' (packed 0xRRGGBB integers: the cue text colour and its backing/outline colour) are required; 'font' is optional (\"shaped\" = real font rasterisation, the default; \"pixel\" = the legacy 3x5 bitmap stamp) and defaults to the font already in force. The reply's 'style' is the style as stored, after clamping. Refused calls (unknown position or font, missing or mistyped arguments) change nothing. A call whose clamped result equals the style already in force is a no-op and spends no undo step; the reply's 'changed' says whether anything moved. One undo step. (Wave P, P29; no-op rule Wave R, R20; font argument Wave RM, RM2)",
            schema: json!({
                "type": "object",
                "properties": {
                    "position": { "type": "string", "enum": ["bottom", "top"], "description": "\"bottom\" or \"top\": the stage edge the subtitle block is anchored to" },
                    "font_scale": { "type": "number", "description": "Font size multiplier, clamped to 0.5-3.0" },
                    "text_color": { "type": "integer", "minimum": 0, "description": "Cue text colour, packed 0xRRGGBB" },
                    "box_color": { "type": "integer", "minimum": 0, "description": "Backing (outline) colour, packed 0xRRGGBB" },
                    "font": { "type": "string", "enum": ["shaped", "pixel"], "description": "Optional raster renderer: \"shaped\" (real font) or \"pixel\" (legacy 3x5 stamp); absent keeps the font in force" }
                },
                "required": ["position", "font_scale", "text_color", "box_color"]
            }),
            handler: tool_subtitle_set_style,
        },
        Tool {
            name: "subtitle_shift_all",
            description: "Shift EVERY subtitle cue by 'ms' milliseconds (the Subtitles panel's Shift-all): the delta converts to frames at the project rate (rounded, and never zero for a non-zero shift), each cue clamps at frame 0 individually with its duration preserved, and the list is re-sorted. The reply's 'moved' counts the cues that actually moved and 'count' the cue total. A shift in which no cue moves changes nothing and spends no undo step. One undo step. (Roadmap Wave 1, RM21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "ms": { "type": "integer", "description": "Shift in milliseconds; negative moves cues earlier" }
                },
                "required": ["ms"]
            }),
            handler: tool_subtitle_shift_all,
        },
        Tool {
            name: "subtitle_retime",
            description: "Retime EVERY subtitle cue from the 'from_fps' rate to the project's current frame rate (the Subtitles panel's Retime cues): each cue's window rescales by the rate ratio so its wall-clock timing is preserved, the half-open minimum is re-established, and the list is re-sorted. The reply's 'retimed' counts the cues in the retimed list and 'count' the cue total; the status names both rates. An equal-rate call, or a rescale that lands every cue where it already sits, changes nothing and spends no undo step. One undo step. (Roadmap Wave 1, RM21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "from_fps": { "type": "number", "description": "The rate the cues are currently timed at (frames per second, greater than 0)" }
                },
                "required": ["from_fps"]
            }),
            handler: tool_subtitle_retime,
        },
    ]
}

// ---------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_named(name: &str) -> Tool {
        tools()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("tool {name} not registered"))
    }

    fn call(name: &str, core: &mut EditorCore, args: Value) -> Result<Value, String> {
        let tool = tool_named(name);
        (tool.handler)(core, &args)
    }

    fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
            speaker: None,
        }
    }

    /// A core whose cue list is seeded directly (the
    /// `mcp_subtitle_io` tests' fixture shape).
    fn cued_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.subtitles = vec![cue(0, 24, "First line"), cue(24, 48, "Second line")];
        core
    }

    #[test]
    fn tools_are_the_contracted_seven_in_order() {
        let names: Vec<&str> = tools().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "subtitle_list",
                "subtitle_add",
                "subtitle_update",
                "subtitle_delete",
                "subtitle_set_style",
                "subtitle_shift_all",
                "subtitle_retime",
            ]
        );
        for t in tools() {
            assert_eq!(t.schema["type"], "object", "tool {} schema", t.name);
        }
    }

    #[test]
    fn list_reports_cues_in_kept_order_and_is_read_only() {
        let mut core = cued_core();
        let status_before = core.status.clone();
        let depth_before = core.undo_depth();
        let reply = call("subtitle_list", &mut core, json!({})).expect("list succeeds");
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(reply["count"], json!(2));
        assert_eq!(reply["subtitles"][0]["index"], json!(0));
        assert_eq!(reply["subtitles"][0]["start_frame"], json!(0));
        assert_eq!(reply["subtitles"][0]["end_frame"], json!(24));
        assert_eq!(reply["subtitles"][0]["text"], json!("First line"));
        assert_eq!(reply["subtitles"][1]["index"], json!(1));
        assert_eq!(reply["subtitles"][1]["text"], json!("Second line"));
        // Read-only: no undo step spent, no status-line
        // write.
        assert_eq!(core.undo_depth(), depth_before);
        assert_eq!(core.status, status_before);
        // An empty project lists as an empty array.
        let mut empty = EditorCore::new();
        let reply = call("subtitle_list", &mut empty, json!({})).expect("empty list succeeds");
        assert_eq!(reply["count"], json!(0));
        assert_eq!(reply["subtitles"], json!([]));
    }

    #[test]
    fn add_inserts_sorted_and_spends_exactly_one_step() {
        let mut core = EditorCore::new();
        let depth_before = core.undo_depth();
        let reply = call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 48, "end_frame": 72, "text": "Later cue" }),
        )
        .expect("add succeeds");
        assert_eq!(reply["index"], json!(0));
        assert_eq!(reply["status"], json!(core.status));
        assert_eq!(core.undo_depth(), depth_before + 1);
        // An earlier cue inserts BEFORE it: the list stays
        // sorted and the reply's index is the new position.
        let reply = call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 0, "end_frame": 24, "text": "Earlier cue" }),
        )
        .expect("second add succeeds");
        assert_eq!(reply["index"], json!(0));
        assert_eq!(core.undo_depth(), depth_before + 2);
        assert_eq!(core.project.subtitles[0].text, "Earlier cue");
        assert_eq!(core.project.subtitles[1].text, "Later cue");
    }

    #[test]
    fn add_refusals_change_nothing_and_spend_nothing() {
        let mut core = EditorCore::new();
        let depth_before = core.undo_depth();
        for args in [
            // Inverted range.
            json!({ "start_frame": 24, "end_frame": 12, "text": "Backwards" }),
            // Empty range (end == start).
            json!({ "start_frame": 24, "end_frame": 24, "text": "Instant" }),
            // Empty and whitespace-only text.
            json!({ "start_frame": 0, "end_frame": 24, "text": "" }),
            json!({ "start_frame": 0, "end_frame": 24, "text": "   " }),
            // Missing argument, refused at parse.
            json!({ "start_frame": 0, "end_frame": 24 }),
            // Negative frame, refused at parse.
            json!({ "start_frame": -1, "end_frame": 24, "text": "Negative" }),
        ] {
            let err = call("subtitle_add", &mut core, args).expect_err("the add refuses");
            assert!(!err.is_empty(), "the refusal explains itself");
        }
        assert!(core.project.subtitles.is_empty());
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn update_replaces_resorts_and_spends_exactly_one_step() {
        let mut core = cued_core();
        let depth_before = core.undo_depth();
        // Move cue 0 past cue 1: the reply's index is the
        // position after the re-sort, not the one addressed.
        let reply = call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 48, "end_frame": 72, "text": "Moved line" }),
        )
        .expect("update succeeds");
        assert_eq!(reply["index"], json!(1));
        assert_eq!(core.undo_depth(), depth_before + 1);
        assert_eq!(core.project.subtitles[0].text, "Second line");
        assert_eq!(core.project.subtitles[1].text, "Moved line");
        assert_eq!(core.project.subtitles[1].start_frame, 48);
    }

    #[test]
    fn update_and_delete_refuse_a_missing_index_without_a_step() {
        let mut core = cued_core();
        let depth_before = core.undo_depth();
        let cues_before = core.project.subtitles.clone();
        call(
            "subtitle_update",
            &mut core,
            json!({ "index": 99, "start_frame": 0, "end_frame": 24, "text": "Ghost" }),
        )
        .expect_err("no such cue refuses the update");
        call("subtitle_delete", &mut core, json!({ "index": 99 }))
            .expect_err("no such cue refuses the delete");
        // A bad range on an existing cue also refuses
        // before any snapshot.
        call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 24, "end_frame": 24, "text": "Instant" }),
        )
        .expect_err("an empty range refuses the update");
        assert_eq!(core.project.subtitles, cues_before);
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn delete_removes_the_cue_and_spends_exactly_one_step() {
        let mut core = cued_core();
        let depth_before = core.undo_depth();
        let reply = call("subtitle_delete", &mut core, json!({ "index": 0 }))
            .expect("delete succeeds");
        assert_eq!(reply["index"], json!(0));
        assert_eq!(reply["status"], json!(core.status));
        assert_eq!(core.undo_depth(), depth_before + 1);
        assert_eq!(core.project.subtitles.len(), 1);
        assert_eq!(core.project.subtitles[0].text, "Second line");
    }

    #[test]
    fn set_style_stores_and_reports_the_resulting_style() {
        let mut core = EditorCore::new();
        let depth_before = core.undo_depth();
        let reply = call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "top",
                "font_scale": 1.5,
                "text_color": 0xFF_FF00u32,
                "box_color": 0x00_0010u32,
            }),
        )
        .expect("set_style succeeds");
        assert_eq!(core.undo_depth(), depth_before + 1);
        // The reply's style is the style as stored.
        assert_eq!(reply["style"]["position"], json!("top"));
        assert_eq!(reply["style"]["font_scale"], json!(1.5));
        assert_eq!(reply["style"]["text_color"], json!(0xFF_FF00u32));
        assert_eq!(reply["style"]["box_color"], json!(0x00_0010u32));
        assert_eq!(
            subtitle_style_json(core.project.subtitle_style),
            reply["style"]
        );
        // A finite out-of-range scale is clamped, not
        // refused — and the reply reports the clamped
        // (resulting) value.
        let reply = call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "bottom",
                "font_scale": 99.0,
                "text_color": 0xFF_FFFFu32,
                "box_color": 0,
            }),
        )
        .expect("an out-of-range scale still succeeds");
        let stored = core.project.subtitle_style.font_scale;
        assert!(stored < 99.0, "the scale was clamped, stored {stored}");
        assert_eq!(reply["style"]["font_scale"], json!(stored));
        assert_eq!(core.undo_depth(), depth_before + 2);
    }

    #[test]
    fn set_style_refusals_change_nothing_and_spend_nothing() {
        let mut core = EditorCore::new();
        let style_before = core.project.subtitle_style;
        let depth_before = core.undo_depth();
        // Unknown position: the parse family's voice.
        let err = call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "middle",
                "font_scale": 1.0,
                "text_color": 0xFF_FFFFu32,
                "box_color": 0,
            }),
        )
        .expect_err("an unknown position refuses");
        assert_eq!(
            err,
            "unknown subtitle position 'middle' (expected \"bottom\" or \"top\")"
        );
        // Missing colour: forced-required at the handler.
        let err = call(
            "subtitle_set_style",
            &mut core,
            json!({ "position": "bottom", "font_scale": 1.0, "box_color": 0 }),
        )
        .expect_err("a missing colour refuses");
        assert_eq!(err, "missing required argument 'text_color'");
        // A colour too large for u32.
        call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "bottom",
                "font_scale": 1.0,
                "text_color": 4_294_967_296u64,
                "box_color": 0,
            }),
        )
        .expect_err("an oversized colour refuses");
        // A finite f64 that overflows f32 (1e300 casts
        // to f32 infinity): refused at parse, before the op.
        let err = call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "bottom",
                "font_scale": 1e300,
                "text_color": 16777215,
                "box_color": 0,
            }),
        )
        .expect_err("an f32-overflowing scale refuses");
        assert_eq!(err, "argument 'font_scale' must be a finite number");
        // An overflowing literal (1e999): if the JSON
        // parser hands it through as f64 infinity, the
        // handler must refuse it the same way; if the
        // parser itself refuses the literal, the value can
        // never reach a handler and there is nothing to
        // prove (the draw_ops precedent).
        if let Ok(args) = serde_json::from_str::<Value>(
            "{\"position\": \"bottom\", \"font_scale\": 1e999, \
             \"text_color\": 16777215, \"box_color\": 0}",
        ) {
            if args["font_scale"]
                .as_f64()
                .map(|f| !f.is_finite())
                .unwrap_or(false)
            {
                let err = call("subtitle_set_style", &mut core, args)
                    .expect_err("a non-finite scale refuses");
                assert_eq!(err, "argument 'font_scale' must be a finite number");
            }
        }
        assert_eq!(core.project.subtitle_style, style_before);
        assert_eq!(core.undo_depth(), depth_before);
    }

    // -----------------------------------------------------------------
    // Wave R (R20): the shell<->MCP parity pins — the
    // style read on `subtitle_list` and the no-change
    // rule on the two replace tools.
    // -----------------------------------------------------------------

    #[test]
    fn list_reports_the_style_in_force() {
        let mut core = cued_core();
        let reply = call("subtitle_list", &mut core, json!({})).expect("list succeeds");
        // The style rides the list reply in the shape
        // set_style echoes — the read half of the panel's
        // read-modify-write style gestures.
        assert_eq!(
            reply["style"],
            subtitle_style_json(core.project.subtitle_style)
        );
        // After a style change the same read reports the
        // new style — and the list stays read-only.
        call(
            "subtitle_set_style",
            &mut core,
            json!({
                "position": "top",
                "font_scale": 1.5,
                "text_color": 0xFF_FF00u32,
                "box_color": 0x00_0010u32,
            }),
        )
        .expect("set_style succeeds");
        let depth_before = core.undo_depth();
        let status_before = core.status.clone();
        let reply = call("subtitle_list", &mut core, json!({})).expect("list succeeds");
        assert_eq!(reply["style"]["position"], json!("top"));
        assert_eq!(reply["style"]["font_scale"], json!(1.5));
        assert_eq!(reply["style"]["text_color"], json!(0xFF_FF00u32));
        assert_eq!(core.undo_depth(), depth_before);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn set_style_noop_spends_nothing_and_reports_unchanged() {
        let mut core = EditorCore::new();
        let args = json!({
            "position": "top",
            "font_scale": 1.5,
            "text_color": 0xFF_FF00u32,
            "box_color": 0x00_0010u32,
        });
        let reply = call("subtitle_set_style", &mut core, args.clone())
            .expect("the first set_style succeeds");
        assert_eq!(reply["changed"], json!(true));
        let depth_before = core.undo_depth();
        // The identical call again: the panel's swatches
        // would never reach the op, and neither does this
        // surface — no step, and the reply says so.
        let reply = call("subtitle_set_style", &mut core, args)
            .expect("the repeat set_style succeeds as a no-op");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(
            reply["style"],
            subtitle_style_json(core.project.subtitle_style)
        );
        assert_eq!(core.undo_depth(), depth_before);
        // The clamp-to-current case: 99.0 stores the max
        // once (a real change), then the same oversized
        // call clamps onto the style in force and is a
        // no-op.
        let big = json!({
            "position": "top",
            "font_scale": 99.0,
            "text_color": 0xFF_FF00u32,
            "box_color": 0x00_0010u32,
        });
        let reply = call("subtitle_set_style", &mut core, big.clone())
            .expect("the clamping set_style succeeds");
        assert_eq!(reply["changed"], json!(true));
        assert_eq!(core.undo_depth(), depth_before + 1);
        let reply = call("subtitle_set_style", &mut core, big)
            .expect("the repeat clamping call succeeds as a no-op");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(core.undo_depth(), depth_before + 1);
    }

    #[test]
    fn update_noop_spends_nothing_and_reports_unchanged() {
        let mut core = cued_core();
        let depth_before = core.undo_depth();
        let cues_before = core.project.subtitles.clone();
        // Cue 0 exactly as stored: the panel's unchanged
        // text apply closes free; this surface reports
        // the no-op and spends nothing.
        let reply = call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "First line" }),
        )
        .expect("the identical update succeeds as a no-op");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(reply["index"], json!(0));
        assert_eq!(core.project.subtitles, cues_before);
        assert_eq!(core.undo_depth(), depth_before);
        // A missing index still refuses through the op's
        // own voice (the guard falls through), and a real
        // update still reports the change and spends
        // exactly one step.
        call(
            "subtitle_update",
            &mut core,
            json!({ "index": 99, "start_frame": 0, "end_frame": 24, "text": "Ghost" }),
        )
        .expect_err("no such cue refuses the update");
        let reply = call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "First line, edited" }),
        )
        .expect("a real update succeeds");
        assert_eq!(reply["changed"], json!(true));
        assert_eq!(core.undo_depth(), depth_before + 1);
    }

    // -----------------------------------------------------------------
    // Wave R (R21): the speaker argument on add/update and
    // its report on list.
    // -----------------------------------------------------------------

    #[test]
    fn speaker_flows_through_add_and_list() {
        let mut core = EditorCore::new();
        let depth_before = core.undo_depth();
        // The name is stored trimmed, and the add spends
        // exactly one step, speaker or not.
        call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 0, "end_frame": 24, "text": "Hello.", "speaker": "  Mary Jane  " }),
        )
        .expect("add with a speaker succeeds");
        assert_eq!(core.undo_depth(), depth_before + 1);
        assert_eq!(
            core.project.subtitles[0].speaker.as_deref(),
            Some("Mary Jane")
        );
        call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 24, "end_frame": 48, "text": "Plain." }),
        )
        .expect("plain add succeeds");
        // The list reports the speaker only where one is
        // carried: an additive field, absent otherwise.
        let reply = call("subtitle_list", &mut core, json!({})).expect("list succeeds");
        assert_eq!(reply["subtitles"][0]["speaker"], json!("Mary Jane"));
        assert!(
            reply["subtitles"][1].get("speaker").is_none(),
            "no speaker key on an uncredited cue: {}",
            reply["subtitles"][1]
        );
        // A name the voice tag cannot carry refuses
        // through the op: no cue, no step.
        let depth_before = core.undo_depth();
        let err = call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 48, "end_frame": 72, "text": "Bad.", "speaker": "A>B" }),
        )
        .expect_err("an unusable speaker name refuses");
        assert!(!err.is_empty(), "the refusal explains itself");
        assert_eq!(core.project.subtitles.len(), 2);
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn update_speaker_legs_follow_the_full_replace_rule() {
        let mut core = EditorCore::new();
        call(
            "subtitle_add",
            &mut core,
            json!({ "start_frame": 0, "end_frame": 24, "text": "Line", "speaker": "Mary" }),
        )
        .expect("add succeeds");
        let depth_before = core.undo_depth();
        // Same range, same text, same speaker: a no-op —
        // the R20 guard compares the speaker too.
        let reply = call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "Line", "speaker": "Mary" }),
        )
        .expect("the identical update succeeds as a no-op");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(core.undo_depth(), depth_before);
        // Omitting the speaker on a speaker'd cue is a
        // real change — the full replace clears it — and
        // spends exactly one step.
        let reply = call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "Line" }),
        )
        .expect("the speaker-clearing update succeeds");
        assert_eq!(reply["changed"], json!(true));
        assert_eq!(core.project.subtitles[0].speaker, None);
        assert_eq!(core.undo_depth(), depth_before + 1);
        // A speaker-only change back is likewise one
        // step; a bad name then refuses through the op
        // and spends nothing.
        call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "Line", "speaker": "Mary" }),
        )
        .expect("restoring the speaker succeeds");
        let depth_before = core.undo_depth();
        call(
            "subtitle_update",
            &mut core,
            json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "Line", "speaker": "A>B" }),
        )
        .expect_err("a bad speaker name refuses");
        assert_eq!(core.project.subtitles[0].speaker.as_deref(), Some("Mary"));
        assert_eq!(core.undo_depth(), depth_before);
    }
}
