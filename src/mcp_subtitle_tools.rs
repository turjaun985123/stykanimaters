//! MCP subtitle cue and style tools (Wave P, P29): the
//! five editing tools over the subtitle core in
//! [`crate::subtitles`] — `subtitle_list`, `subtitle_add`,
//! `subtitle_update`, `subtitle_delete` and
//! `subtitle_set_style`. (The SRT/WebVTT file exchange
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

use crate::editor::EditorCore;
use crate::mcp::Tool;
use crate::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};
use serde_json::{json, Value};

// ---------------------------------------------------------------------
// Argument helpers (the `mcp_tools` shapes, duplicated the
// way this tree's MCP modules duplicate their small
// helpers — same refusal sentences, including the Wave 11
// finiteness rule: a JSON number like 1e999 parses to f64
// infinity, and a finite f64 like 1e300 still casts to f32
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

fn sub_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match sub_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
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
fn subtitle_cue_json(index: usize, cue: &SubtitleCue) -> Value {
    json!({
        "index": index,
        "start_frame": cue.start_frame,
        "end_frame": cue.end_frame,
        "text": cue.text,
    })
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
    }))
}

fn tool_subtitle_add(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let start_frame = sub_req_usize(args, "start_frame")?;
    let end_frame = sub_req_usize(args, "end_frame")?;
    let text = sub_req_str(args, "text")?;
    match core.subtitle_add(start_frame, end_frame, text) {
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
    match core.subtitle_update(index, start_frame, end_frame, text) {
        // The op re-sorts after the replace: the reply's
        // index is the cue's position in the list now,
        // which may differ from the index addressed.
        Some(new_index) => Ok(json!({
            "ok": true,
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
    let style = SubtitleStyle {
        position,
        font_scale,
        text_color,
        box_color,
    };
    // The op validates, snapshots exactly once on success
    // (one undo step) and explains a refusal on the status
    // line.
    core.set_subtitle_style(style)
        .map_err(|_| core.status.clone())?;
    Ok(json!({
        "ok": true,
        "style": subtitle_style_json(core.project.subtitle_style),
        "status": core.status,
        "project": sub_project_summary(core),
    }))
}

// ---------------------------------------------------------------------
// registration — NOTE: this module's tools() is NOT yet
// merged into `mcp::all_tools()`; that wiring (plus the
// lib.rs module line) is an integrator action filed in
// the P29 findings.
// ---------------------------------------------------------------------

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "subtitle_list",
            description: "List the project's subtitle cues in their kept order (sorted by start frame, then end frame): each cue's index in the list, its half-open frame range [start_frame, end_frame) and its text. Indices are 0-based, the convention of every index in this registry. Read-only. (Wave P, P29)",
            schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            handler: tool_subtitle_list,
        },
        Tool {
            name: "subtitle_add",
            description: "Add a subtitle cue over the half-open frame range [start_frame, end_frame) with the given text: the cue shows at start_frame and is gone at end_frame, so end_frame must be after start_frame and the text must not be empty. The cue list is kept sorted by (start_frame, end_frame) and the reply's 'index' is the new cue's position in it. Frames are 0-based and are not clamped to the timeline. Refused cues (bad range, empty text) change nothing. One undo step. (Wave P, P29)",
            schema: json!({
                "type": "object",
                "properties": {
                    "start_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is shown on (0-based)" },
                    "end_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is no longer shown on (0-based, exclusive; must be after start_frame)" },
                    "text": { "type": "string", "description": "The cue's text (must not be empty)" }
                },
                "required": ["start_frame", "end_frame", "text"]
            }),
            handler: tool_subtitle_add,
        },
        Tool {
            name: "subtitle_update",
            description: "Replace the subtitle cue at 'index' (its index in subtitle_list) with a new half-open frame range [start_frame, end_frame) and text — a full replace, all four arguments required. The list is re-sorted afterwards and the reply's 'index' is the cue's position in the list now, which may differ from the index addressed. Refused updates (no such cue, bad range, empty text) change nothing. One undo step. (Wave P, P29)",
            schema: json!({
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "minimum": 0, "description": "Index of the cue in subtitle_list" },
                    "start_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is shown on (0-based)" },
                    "end_frame": { "type": "integer", "minimum": 0, "description": "First frame the cue is no longer shown on (0-based, exclusive; must be after start_frame)" },
                    "text": { "type": "string", "description": "The cue's text (must not be empty)" }
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
            description: "Replace the project's subtitle style in one call — a full replace, all four arguments required: 'position' (\"bottom\" or \"top\", the stage edge the cue block is anchored to), 'font_scale' (multiplier on the renderers' house subtitle size; a finite value is clamped to 0.5-3.0, a non-finite one is refused), 'text_color' and 'box_color' (packed 0xRRGGBB integers: the cue text colour and its backing/outline colour). The reply's 'style' is the style as stored, after clamping. Refused calls (unknown position, missing or mistyped arguments) change nothing. One undo step. (Wave P, P29)",
            schema: json!({
                "type": "object",
                "properties": {
                    "position": { "type": "string", "enum": ["bottom", "top"], "description": "\"bottom\" or \"top\": the stage edge the subtitle block is anchored to" },
                    "font_scale": { "type": "number", "description": "Font size multiplier, clamped to 0.5-3.0" },
                    "text_color": { "type": "integer", "minimum": 0, "description": "Cue text colour, packed 0xRRGGBB" },
                    "box_color": { "type": "integer", "minimum": 0, "description": "Backing (outline) colour, packed 0xRRGGBB" }
                },
                "required": ["position", "font_scale", "text_color", "box_color"]
            }),
            handler: tool_subtitle_set_style,
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
    fn tools_are_the_contracted_five_in_order() {
        let names: Vec<&str> = tools().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "subtitle_list",
                "subtitle_add",
                "subtitle_update",
                "subtitle_delete",
                "subtitle_set_style",
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
        // A non-finite scale (1e999 parses to f64
        // infinity): refused at parse, before the op.
        let args: Value = serde_json::from_str(
            "{\"position\": \"bottom\", \"font_scale\": 1e999, \
             \"text_color\": 16777215, \"box_color\": 0}",
        )
        .unwrap();
        let err = call("subtitle_set_style", &mut core, args)
            .expect_err("a non-finite scale refuses");
        assert_eq!(err, "argument 'font_scale' must be a finite number");
        assert_eq!(core.project.subtitle_style, style_before);
        assert_eq!(core.undo_depth(), depth_before);
    }
}
