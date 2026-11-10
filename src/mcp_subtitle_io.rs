//! MCP subtitle file exchange (Wave P, P30): the SRT and
//! WebVTT import/export tools — the path-based twins over the
//! converters in [`crate::subtitles`].
//!
//! The discipline is the settled path-based IO contract (the
//! same one the `mcp_io` family follows):
//!
//! - Export takes a required `path`, serialises the project's
//!   cues at the project's frame rate and writes the document.
//!   It is read-only: no snapshot (no undo step), no status-line
//!   write — the success sentence travels in the reply's
//!   `status` field only. A path that cannot be written is
//!   refused and changes nothing.
//! - Import reads the file and parses it BEFORE anything
//!   mutates: an unreadable file or a document in which
//!   nothing parses is refused with the reader's/parser's own
//!   error, and the project is untouched. A successful parse
//!   replaces the whole cue list, re-sorted into the kept
//!   order (`sort_subtitle_cues`). An import identical to the
//!   current list reports `changed: false` and spends NO undo
//!   step; a changed list spends exactly one (`snapshot()`
//!   before the replace). The reply carries `changed` and the
//!   cue `count` either way.
//!
//! There is deliberately no core op for the bulk replace:
//! these handlers are the MCP surface, mirroring the GUI
//! import/export behaviour directly, the way `mcp_io`'s
//! handlers mirror the editor's save/load.
//!
//! Cue speakers (Wave R, R21) ride the converters, so
//! they ride these tools unchanged: WebVTT voice tags
//! round-trip them, and SRT neither writes nor reads
//! them (the converters' documented settlement — an SRT
//! import into a speaker'd project therefore reports
//! `changed: true` and the imported cues carry no
//! speaker). The loss is also surfaced at export time
//! (Wave S, S9): an SRT export whose cues carry
//! speakers reports the unwritten count in its reply
//! as `speakers_dropped`, present only when above
//! zero.

use crate::editor::EditorCore;
use crate::mcp::Tool;
use crate::subtitles::{
    subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt_counted, subtitles_to_vtt,
};
use serde_json::{json, Value};
use std::path::Path;

// ---------------------------------------------------------------------
// Argument / filesystem helpers (the `mcp_io` shapes, duplicated
// the way this tree's MCP modules duplicate their small helpers)
// ---------------------------------------------------------------------

/// Required `path` argument: missing, non-string or empty all
/// refuse with the `mcp_io` family's sentence.
fn sio_arg_path(args: &Value) -> Result<String, String> {
    args.get("path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "missing required string argument: path".to_string())
}

fn sio_ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create directory {}: {e}", parent.display()))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// export (read-only)
// ---------------------------------------------------------------------

/// The WebVTT export body: serialise with `to_document`, write
/// to `path`, reply with the count and the success sentence. No
/// snapshot, no `core.status` write — the tool is read-only.
/// (The SRT handler mirrors this body rather than sharing it:
/// its reply also surfaces the dropped speaker count — see
/// `tool_subtitle_export_srt`.)
fn export_document(
    core: &mut EditorCore,
    args: &Value,
    to_document: fn(&crate::model::Project) -> String,
) -> Result<Value, String> {
    let path = sio_arg_path(args)?;
    let path_ref = Path::new(&path);
    sio_ensure_parent(path_ref)?;
    let text = to_document(&core.project);
    std::fs::write(path_ref, &text)
        .map_err(|e| format!("could not write {}: {e}", path_ref.display()))?;
    let count = core.project.subtitles.len();
    Ok(json!({
        "ok": true,
        "path": path,
        "count": count,
        "status": format!("Exported {count} subtitle cue(s) to {path}"),
    }))
}

fn tool_subtitle_export_srt(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    // The counted export (Wave S, S9): this handler
    // mirrors `export_document`'s body rather than
    // sharing it, because the reply — not the document
    // — is where the two formats differ: SRT drops cue
    // speakers, and when it dropped any, the reply says
    // how many. The field is additive and present ONLY
    // above zero, so an uncredited export's reply keeps
    // exactly its established shape.
    let path = sio_arg_path(args)?;
    let path_ref = Path::new(&path);
    sio_ensure_parent(path_ref)?;
    let export = subtitles_to_srt_counted(&core.project);
    std::fs::write(path_ref, &export.text)
        .map_err(|e| format!("could not write {}: {e}", path_ref.display()))?;
    let count = core.project.subtitles.len();
    let mut reply = json!({
        "ok": true,
        "path": path,
        "count": count,
        "status": format!("Exported {count} subtitle cue(s) to {path}"),
    });
    if export.speakers_dropped > 0 {
        reply["speakers_dropped"] = json!(export.speakers_dropped);
    }
    Ok(reply)
}

fn tool_subtitle_export_vtt(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    export_document(core, args, subtitles_to_vtt)
}

// ---------------------------------------------------------------------
// import (one undo step when changed, none when identical)
// ---------------------------------------------------------------------

/// Shared import body: read + parse before any mutation,
/// re-sort, then the changed/identical fork. `format_label`
/// is the sentence fragment ("SRT" / "WebVTT") the status
/// lines use.
fn import_document(
    core: &mut EditorCore,
    args: &Value,
    from_document: fn(&str, f32) -> Result<Vec<crate::subtitles::SubtitleCue>, String>,
    format_label: &str,
) -> Result<Value, String> {
    let path = sio_arg_path(args)?;
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))?;
    // The parse runs before anything mutates: a document in
    // which nothing parses refuses here, with the parser's
    // own error, and the project is untouched.
    let mut cues = from_document(&text, core.project.fps)?;
    crate::subtitles::sort_subtitle_cues(&mut cues);
    if cues == core.project.subtitles {
        core.status =
            format!("{format_label} import matched the current subtitles; nothing changed.");
        return Ok(json!({
            "ok": true,
            "path": path,
            "count": cues.len(),
            "changed": false,
            "status": core.status,
        }));
    }
    let count = cues.len();
    core.snapshot();
    core.project.subtitles = cues;
    core.status = format!("Imported {count} subtitle(s) from {format_label}.");
    Ok(json!({
        "ok": true,
        "path": path,
        "count": count,
        "changed": true,
        "status": core.status,
    }))
}

fn tool_subtitle_import_srt(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    import_document(core, args, subtitles_from_srt, "SRT")
}

fn tool_subtitle_import_vtt(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    import_document(core, args, subtitles_from_vtt, "WebVTT")
}

// ---------------------------------------------------------------------
// registration — wired into `mcp::all_tools()` since
// Wave Q (Q1): the lib.rs module line and the all_tools
// extend are in `src/mcp.rs`, before the cue/style
// family's extend.
// ---------------------------------------------------------------------

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "subtitle_export_srt",
            description: "Write the project's subtitle cues as an SRT (SubRip) document to the file at 'path', timed at the project's frame rate (the Subtitles panel's SRT export). Cue speakers are not written — SRT has no speaker syntax — and cue text is written verbatim. When cues carry speakers, the reply counts the unwritten credits in 'speakers_dropped' (present only when above zero). Read-only; no undo step. A path that cannot be written is refused and changes nothing. (Wave P, P30; speaker note Wave R, R21; speakers_dropped surfacing Wave S, S9)",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination .srt file path" }
                },
                "required": ["path"]
            }),
            handler: tool_subtitle_export_srt,
        },
        Tool {
            name: "subtitle_import_srt",
            description: "Replace the project's whole subtitle cue list with the cues parsed from the SRT (SubRip) file at 'path', timed at the project's frame rate (the Subtitles panel's SRT import): timestamps convert to half-open frame ranges and the list is kept sorted. Imported cues carry no speaker: SRT has none to read. A file that cannot be read, or in which nothing parses, is refused before any change. An import matching the current list changes nothing and spends no undo step; a changed list is one undo step. The reply's 'changed' says which happened. (Wave P, P30; speaker note Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source .srt file path" }
                },
                "required": ["path"]
            }),
            handler: tool_subtitle_import_srt,
        },
        Tool {
            name: "subtitle_export_vtt",
            description: "Write the project's subtitle cues as a WebVTT document to the file at 'path', timed at the project's frame rate (the Subtitles panel's WebVTT export). A cue's speaker is written as a WebVTT voice tag (<v Name>…</v>) around its text. Read-only; no undo step. A path that cannot be written is refused and changes nothing. (Wave P, P30; speaker Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Destination .vtt file path" }
                },
                "required": ["path"]
            }),
            handler: tool_subtitle_export_vtt,
        },
        Tool {
            name: "subtitle_import_vtt",
            description: "Replace the project's whole subtitle cue list with the cues parsed from the WebVTT file at 'path', timed at the project's frame rate (the Subtitles panel's WebVTT import): timestamps convert to half-open frame ranges and the list is kept sorted. WebVTT voice tags are read back as cue speakers; a malformed tag degrades to plain cue text, never a lost cue. A file that cannot be read, or with no WEBVTT header, or in which nothing parses, is refused before any change. An import matching the current list changes nothing and spends no undo step; a changed list is one undo step. The reply's 'changed' says which happened. (Wave P, P30; speaker Wave R, R21)",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source .vtt file path" }
                },
                "required": ["path"]
            }),
            handler: tool_subtitle_import_vtt,
        },
    ]
}

// ---------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::{subtitles_to_srt, SubtitleCue};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("styk-mcp-subtitle-io-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    /// A core at 24 fps (set explicitly — never the demo
    /// rate) with two stored cues.
    fn cued_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.subtitles = vec![
            SubtitleCue {
                start_frame: 0,
                end_frame: 24,
                text: "First line".to_string(),
                speaker: None,
            },
            SubtitleCue {
                start_frame: 24,
                end_frame: 48,
                text: "Second line".to_string(),
                speaker: None,
            },
        ];
        core
    }

    #[test]
    fn export_srt_writes_the_document_and_is_read_only() {
        let dir = temp_dir("export-srt");
        let path = dir.join("cues.srt");
        let mut core = cued_core();
        let status_before = core.status.clone();
        let depth_before = core.undo_depth();
        let reply = call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("export succeeds");
        assert_eq!(reply["count"], json!(2));
        assert_eq!(reply["ok"], json!(true));
        let written = std::fs::read_to_string(&path).expect("the file was written");
        assert_eq!(written, subtitles_to_srt(&core.project));
        assert!(written.contains("00:00:00,000 --> 00:00:01,000"));
        // Read-only: no undo step spent, no status-line write.
        assert_eq!(core.undo_depth(), depth_before);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn export_srt_reports_dropped_speakers_only_when_positive() {
        // Wave S (S9): the reply gains `speakers_dropped`
        // only when the export actually dropped credits.
        let dir = temp_dir("export-srt-speakers");
        let path = dir.join("cues.srt");
        // Uncredited cues (the cued_core fixture): the
        // field is absent and the reply keeps exactly
        // its established shape.
        let mut core = cued_core();
        let reply = call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("uncredited export succeeds");
        assert!(
            reply.get("speakers_dropped").is_none(),
            "no field when nothing was dropped: {reply}"
        );
        assert_eq!(reply["count"], json!(2));
        // Two credited cues: the field reports both, and
        // the document on disk is the plain export's.
        core.project.subtitles[0].speaker = Some("Mary Jane".to_string());
        core.project.subtitles[1].speaker = Some("Tom".to_string());
        let reply = call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("credited export succeeds");
        assert_eq!(reply["speakers_dropped"], json!(2));
        assert_eq!(reply["count"], json!(2));
        let written = std::fs::read_to_string(&path).expect("the file was written");
        assert_eq!(written, subtitles_to_srt(&core.project));
        assert!(!written.contains("Mary"), "no name in the SRT: {written}");
        // One credit among plain cues: exactly one.
        core.project.subtitles[1].speaker = None;
        let reply = call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("single-credit export succeeds");
        assert_eq!(reply["speakers_dropped"], json!(1));
        // The WebVTT twin never carries the field — it
        // drops nothing.
        let vtt_path = dir.join("cues.vtt");
        let reply = call(
            "subtitle_export_vtt",
            &mut core,
            json!({ "path": vtt_path.to_string_lossy() }),
        )
        .expect("vtt export succeeds");
        assert!(
            reply.get("speakers_dropped").is_none(),
            "the VTT reply has no speakers_dropped: {reply}"
        );
    }

    #[test]
    fn export_vtt_writes_a_webvtt_document() {
        let dir = temp_dir("export-vtt");
        let path = dir.join("cues.vtt");
        let mut core = cued_core();
        let reply = call(
            "subtitle_export_vtt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("export succeeds");
        assert_eq!(reply["count"], json!(2));
        let written = std::fs::read_to_string(&path).expect("the file was written");
        assert!(written.starts_with("WEBVTT"), "vtt document: {written}");
        assert_eq!(written, subtitles_to_vtt(&core.project));
    }

    #[test]
    fn export_refuses_an_unwritable_path_and_a_missing_path() {
        let dir = temp_dir("export-refuse");
        // A path whose parent is a regular file cannot be
        // created or written.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();
        let bad = blocker.join("cues.srt");
        let mut core = cued_core();
        let depth_before = core.undo_depth();
        let err = call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": bad.to_string_lossy() }),
        )
        .expect_err("an unwritable path refuses");
        assert!(err.contains("could not"), "refusal voice: {err}");
        assert_eq!(core.undo_depth(), depth_before);
        assert_eq!(core.project.subtitles.len(), 2);
        // Missing/empty path: the mcp_io family's sentence.
        let err = call("subtitle_export_srt", &mut core, json!({}))
            .expect_err("a missing path refuses");
        assert_eq!(err, "missing required string argument: path");
        let err = call("subtitle_import_vtt", &mut core, json!({ "path": "" }))
            .expect_err("an empty path refuses");
        assert_eq!(err, "missing required string argument: path");
    }

    #[test]
    fn import_srt_changes_once_and_matches_for_free() {
        let dir = temp_dir("import-srt");
        let path = dir.join("in.srt");
        std::fs::write(
            &path,
            "1\n00:00:01,000 --> 00:00:02,000\nLater cue\n\n\
             2\n00:00:00,000 --> 00:00:01,000\nEarlier cue\n",
        )
        .unwrap();
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        let depth_before = core.undo_depth();
        let reply = call(
            "subtitle_import_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("import succeeds");
        assert_eq!(reply["changed"], json!(true));
        assert_eq!(reply["count"], json!(2));
        // Exactly one undo step for the whole-list replace.
        assert_eq!(core.undo_depth(), depth_before + 1);
        // Kept sorted by (start_frame, end_frame).
        assert_eq!(core.project.subtitles[0].text, "Earlier cue");
        assert_eq!(core.project.subtitles[1].text, "Later cue");
        assert_eq!(core.project.subtitles[0].start_frame, 0);
        assert_eq!(core.project.subtitles[1].start_frame, 24);
        // The identical import again: changed == false, no
        // further undo step.
        let reply = call(
            "subtitle_import_srt",
            &mut core,
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("second import succeeds");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(core.undo_depth(), depth_before + 1);
    }

    #[test]
    fn import_refusals_change_nothing() {
        let dir = temp_dir("import-refuse");
        let mut core = cued_core();
        let cues_before = core.project.subtitles.clone();
        let depth_before = core.undo_depth();
        // Unreadable file.
        let missing = dir.join("nope.srt");
        let err = call(
            "subtitle_import_srt",
            &mut core,
            json!({ "path": missing.to_string_lossy() }),
        )
        .expect_err("an unreadable file refuses");
        assert!(err.contains("could not read"), "refusal voice: {err}");
        // Zero-yield document: parses to nothing, refused
        // before any mutation with the parser's own error.
        let junk = dir.join("junk.srt");
        std::fs::write(&junk, "nothing parseable\n").unwrap();
        call(
            "subtitle_import_srt",
            &mut core,
            json!({ "path": junk.to_string_lossy() }),
        )
        .expect_err("a zero-yield document refuses");
        // Headerless WebVTT: the one whole-file refusal.
        let headerless = dir.join("junk.vtt");
        std::fs::write(
            &headerless,
            "00:00:00.000 --> 00:00:01.000\nNo header\n",
        )
        .unwrap();
        call(
            "subtitle_import_vtt",
            &mut core,
            json!({ "path": headerless.to_string_lossy() }),
        )
        .expect_err("a headerless WebVTT file refuses");
        assert_eq!(core.project.subtitles, cues_before);
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn srt_export_then_vtt_import_round_trips() {
        let dir = temp_dir("cross");
        let srt_path = dir.join("cues.srt");
        let vtt_path = dir.join("cues.vtt");
        let mut core = cued_core();
        call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": srt_path.to_string_lossy() }),
        )
        .unwrap();
        call(
            "subtitle_export_vtt",
            &mut core,
            json!({ "path": vtt_path.to_string_lossy() }),
        )
        .unwrap();
        // Importing either document back into the same core
        // is a no-change import: frame-exact at 24 fps.
        for (tool, path) in [
            ("subtitle_import_srt", &srt_path),
            ("subtitle_import_vtt", &vtt_path),
        ] {
            let reply = call(tool, &mut core, json!({ "path": path.to_string_lossy() }))
                .expect("round-trip import succeeds");
            assert_eq!(reply["changed"], json!(false), "{tool} round trip");
            assert_eq!(reply["count"], json!(2));
        }
    }

    #[test]
    fn vtt_round_trip_keeps_the_speaker_and_srt_drops_it() {
        // Wave R (R21) at the tool surface: the speaker
        // rides WebVTT's voice tag end to end, while SRT
        // drops it — visibly, as a changed import whose
        // text is untouched.
        let dir = temp_dir("speaker");
        let vtt_path = dir.join("cues.vtt");
        let srt_path = dir.join("cues.srt");
        let mut core = cued_core();
        core.project.subtitles[0].speaker = Some("Mary Jane".to_string());
        call(
            "subtitle_export_vtt",
            &mut core,
            json!({ "path": vtt_path.to_string_lossy() }),
        )
        .unwrap();
        let written = std::fs::read_to_string(&vtt_path).unwrap();
        assert!(
            written.contains("<v Mary Jane>First line</v>"),
            "the voice tag is on the wire: {written}"
        );
        // Back into the same core: a no-change import,
        // speaker included in the comparison.
        let reply = call(
            "subtitle_import_vtt",
            &mut core,
            json!({ "path": vtt_path.to_string_lossy() }),
        )
        .expect("vtt round trip succeeds");
        assert_eq!(reply["changed"], json!(false));
        assert_eq!(
            core.project.subtitles[0].speaker.as_deref(),
            Some("Mary Jane")
        );
        // SRT: the document carries no name, and importing
        // it is a real change whose only casualty is the
        // speaker — frames and text identical.
        call(
            "subtitle_export_srt",
            &mut core,
            json!({ "path": srt_path.to_string_lossy() }),
        )
        .unwrap();
        let written = std::fs::read_to_string(&srt_path).unwrap();
        assert!(!written.contains("Mary"), "no name in the SRT: {written}");
        let reply = call(
            "subtitle_import_srt",
            &mut core,
            json!({ "path": srt_path.to_string_lossy() }),
        )
        .expect("srt import succeeds");
        assert_eq!(reply["changed"], json!(true), "the speaker loss is a change");
        assert_eq!(core.project.subtitles[0].speaker, None);
        assert_eq!(core.project.subtitles[0].text, "First line");
        assert_eq!(core.project.subtitles[0].start_frame, 0);
        assert_eq!(core.project.subtitles[0].end_frame, 24);
    }
}
