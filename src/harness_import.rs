//! Harness bridge (Roadmap Wave 2, W2-13 — E-4): import an
//! Ultimate Harness `story.json` into a StykAnimate-RS
//! project.
//!
//! The harness (the user's stickman episode pipeline,
//! `~/workspace/animation-project/stickman-system/harness/`)
//! authors episodes as one small JSON file: beats timed in
//! SECONDS, dialogue lines, chapter titles, a cast named by
//! Character Bible ids. This module is the one-way bridge
//! from that file to this editor's project model:
//!
//! * beat durations × the project fps → authored frame
//!   ranges; the timeline is one rest-pose frame per frame
//!   (performance data — actions, expressions, gazes — has
//!   no pose-track home yet and is reported, never faked);
//! * a beat's `chapter` string opens a [`Chapter`] running
//!   to the next chapter beat or the end of the timeline
//!   (W2-11's model, story order preserved);
//! * every dialogue line becomes a [`SubtitleCue`] with its
//!   text VERBATIM and its speaker resolved to the cast
//!   member's Bible display name — dialogue is never
//!   dropped, trimmed or paraphrased (the standing user
//!   rule); a line's cue is placed at the harness compiler's
//!   own spoken-length estimate (2.4 words/s + 0.25 s, the
//!   `compile.py` constants) inside its beat;
//! * each cast member becomes a figure built from their
//!   Character Bible's `design` block: the Bible's segment
//!   lengths rescaled onto the project stage, its ink as
//!   the figure colour, its head/limb colours as node
//!   colour overrides — always the classic 11-node stickman
//!   topology, because the cast stays recognisably stickman
//!   (the channel rule). A cast member with no Bible gets
//!   a plainly-reported placeholder humanoid, never a
//!   silent invention.
//!
//! Honesty contract: everything the story or a Bible
//! carries that this model cannot hold is listed, field by
//! field, in [`ImportReport::unmapped`] — shots, actions,
//! expressions, sfx, VFX, accessories, head geometry, Bible
//! prose. The import refuses (with no project produced)
//! only when there is nothing to import: unparseable JSON,
//! no beats, or beats totalling zero frames.
//!
//! Story shapes accepted: v2 (`set`/`look`, per-actor keys,
//! `line`/`lines`), v3+ (`environment`, `actors`,
//! `dialogue`, `events`) and v7 (chapters, per-beat
//! set/look) — the same union the harness's own
//! `normalize_story` accepts. Bibles are supplied by the
//! caller as parsed JSON values (one per
//! `harness/characters/*.json` file); the importer itself
//! does no file IO.
//!
//! Integrator wiring: declare `pub mod harness_import;` in
//! `lib.rs` (alphabetical: after `pub mod graph_panel;`,
//! before `pub mod i18n;`) — IA-W2-13-1. The panel entry
//! and the MCP tool are IA-W2-13-2 / IA-W2-13-3. Until
//! those land this module is compiled by its own tests
//! and nothing in the live editor changes.

use serde_json::Value;

use crate::editor::EditorCore;
use crate::model::{Chapter, FigureDef, FigureInstance, Frame, Project};
use crate::subtitles::{normalise_speaker, SubtitleCue};

/// The harness compiler's measured speech pace
/// (`compile.py`: `WORDS_PER_SECOND`), words per second.
const WORDS_PER_SECOND: f64 = 2.4;
/// The harness compiler's per-line settle time
/// (`compile.py`: `line_seconds`' constant term).
const LINE_SETTLE_S: f64 = 0.25;
/// Pause the compiler leaves after each spoken line
/// (`compile.py`: `LINE_PAD_S`).
const LINE_PAD_S: f64 = 0.35;
/// The harness render contract's frame rate, used when a
/// story carries no `fps` of its own (v2 stories).
const DEFAULT_STORY_FPS: f64 = 30.0;
/// The harness design space's height: Bibles are authored
/// against the 1080×1920 vertical render contract, so a
/// design length scales by `stage_height / 1920`.
const DESIGN_SPACE_HEIGHT: f64 = 1920.0;
/// Horizontal distance between neighbouring cast members'
/// rest roots on the imported stage.
const CAST_SPACING: f32 = 180.0;

/// What an import did — the honest account a calling
/// surface (panel, MCP tool) shows the user. Counts say
/// what landed; `unmapped` says, field by field, what the
/// story carried that this project model cannot hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    /// The story's `title` (also the project's name).
    pub title: String,
    /// The story's `version`, when it carries one.
    pub source_version: Option<i64>,
    /// The frame rate the beats were converted at (the
    /// story's `fps`, or 30 when it carries none).
    pub fps: f32,
    /// Beats read from the story.
    pub beats: usize,
    /// Authored frames produced (the timeline length).
    pub frames: usize,
    /// Chapters opened by chapter beats.
    pub chapters: usize,
    /// Dialogue lines imported as subtitle cues — every
    /// non-empty line in the story, verbatim.
    pub cues: usize,
    /// Figure display names, in cast order.
    pub figures: Vec<String>,
    /// Cast ids with no Character Bible: each still got a
    /// figure (a default humanoid), and is named here so
    /// the substitution is never silent.
    pub missing_bibles: Vec<String>,
    /// Bible documents the caller supplied that did not
    /// parse as JSON (the `_json` entry point only).
    pub bible_errors: Vec<String>,
    /// Fields the story/Bibles carried that were NOT
    /// mapped, one entry per field family, with the
    /// reason. Never contains dialogue: lines always map.
    pub unmapped: Vec<String>,
    /// Factual notes about choices the import made
    /// (defaults taken, lines overrunning their beat,
    /// speakers normalised).
    pub notes: Vec<String>,
}

/// A finished import: the project plus its report.
#[derive(Clone, Debug)]
pub struct ImportedStory {
    pub project: Project,
    pub report: ImportReport,
}

fn push_unique(list: &mut Vec<String>, entry: String) {
    if !list.contains(&entry) {
        list.push(entry);
    }
}

/// Append a cast id once, in first-appearance order (a
/// free function, not a closure: the cast scan interleaves
/// immutable reads of the list with these appends).
fn add_cast_id(cast_ids: &mut Vec<String>, id: &str) {
    if !id.is_empty() && !cast_ids.iter().any(|c| c == id) {
        cast_ids.push(id.to_string());
    }
}

/// Parse a `#RRGGBB` (or bare `RRGGBB`) colour into the
/// model's `0xRRGGBB` form. Anything else is `None`.
fn parse_hex_color(raw: &str) -> Option<u32> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn num_field(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

// ----------------------------------------------------------------
// Character Bibles → figures
// ----------------------------------------------------------------

/// One Bible's `design` block, read leniently: every
/// number is optional and falls back to the classic
/// humanoid's proportions, because Bibles in the wild
/// vary (one current Bible omits `legEnd` entirely).
struct BibleDesign {
    ink: Option<u32>,
    head_color: Option<u32>,
    limb_color: Option<u32>,
    torso: Option<f64>,
    neck: Option<f64>,
    upper_arm: Option<f64>,
    fore_arm: Option<f64>,
    leg_upper: Option<f64>,
    leg_lower: Option<f64>,
    limb_width: Option<f64>,
}

impl BibleDesign {
    fn read(bible: &Value) -> Option<Self> {
        let design = bible.get("design")?;
        if !design.is_object() {
            return None;
        }
        let color = |key: &str| {
            str_field(design, key).and_then(parse_hex_color)
        };
        Some(BibleDesign {
            ink: color("ink"),
            head_color: color("headColor"),
            limb_color: color("limbColor"),
            torso: num_field(design, "torso"),
            neck: num_field(design, "neck"),
            upper_arm: num_field(design, "upperArm"),
            fore_arm: num_field(design, "foreArm"),
            leg_upper: num_field(design, "legUpper"),
            leg_lower: num_field(design, "legLower"),
            limb_width: num_field(design, "limbWidth"),
        })
    }
}

/// A cast member resolved for import: the figure to add
/// plus the bookkeeping the report needs.
struct CastMember {
    id: String,
    name: String,
    def: FigureDef,
    color: u32,
}

/// Build the stickman figure for one Bible at `scale`
/// (design units → stage units). The topology is ALWAYS
/// `FigureDef::humanoid`'s 11 nodes — same parents, same
/// rest angles, same joint limits — with the Bible's
/// lengths, stroke width and colours laid over it, so a
/// Bible changes a stickman's proportions and palette,
/// never its species.
fn figure_from_bible(
    id: &str,
    bible: &Value,
    scale: f32,
    report: &mut ImportReport,
) -> CastMember {
    let name = str_field(bible, "name")
        .map(str::to_string)
        .unwrap_or_else(|| id.to_string());
    let design = BibleDesign::read(bible);
    let mut def = FigureDef::humanoid();
    let humanoid = FigureDef::humanoid();
    // A design length, scaled, falling back to the
    // humanoid's own length for that node.
    let seg = |value: Option<f64>, node: usize| -> f32 {
        value
            .map(|v| (v as f32) * scale)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(humanoid.nodes[node].length)
    };
    if let Some(d) = &design {
        // The Bible's `torso` runs hips→neck and its
        // `neck` neck→head-centre; the model's chest node
        // is the difference, its head node the neck.
        let chest = match (d.torso, d.neck) {
            (Some(t), Some(n)) => Some(t - n),
            (Some(t), None) => Some(t),
            _ => None,
        };
        def.nodes[1].length = seg(chest, 1);
        def.nodes[2].length = seg(d.neck, 2);
        for node in [3, 5] {
            def.nodes[node].length = seg(d.upper_arm, node);
        }
        for node in [4, 6] {
            def.nodes[node].length = seg(d.fore_arm, node);
        }
        for node in [7, 9] {
            def.nodes[node].length = seg(d.leg_upper, node);
        }
        for node in [8, 10] {
            def.nodes[node].length = seg(d.leg_lower, node);
        }
        if let Some(w) = d.limb_width.map(|v| (v as f32) * scale) {
            if w.is_finite() && w > 0.0 {
                for node in def.nodes.iter_mut().skip(1) {
                    node.thickness = w;
                }
            }
        }
        // Colours: ink is the figure colour; a differing
        // head or limb colour rides as a node override.
        if let Some(head) = d.head_color {
            if Some(head) != d.ink {
                def.nodes[2].color = Some(head);
            }
        }
        if let Some(limb) = d.limb_color {
            if Some(limb) != d.ink {
                for node in [3, 4, 5, 6, 7, 8, 9, 10] {
                    def.nodes[node].color = Some(limb);
                }
            }
        }
    }
    let color = design
        .as_ref()
        .and_then(|d| d.ink.or(d.limb_color))
        .unwrap_or(0xE8E8EC);

    // The Bible fields with no model home, named per
    // figure so the report never hides a substitution.
    if let Some(accessory) = str_field(bible, "accessory") {
        push_unique(
            &mut report.unmapped,
            format!(
                "{name}: accessory '{accessory}' — figures have no accessory slot; \
                 not drawn (segment media could dress the node in a later pass)"
            ),
        );
    }
    if let Some(design_v) = bible.get("design") {
        let head_keys = [
            "headRadius",
            "headSquashY",
            "eyeSpacing",
            "eyeScale",
            "headLineWidth",
        ];
        if head_keys.iter().any(|k| design_v.get(k).is_some()) {
            push_unique(
                &mut report.unmapped,
                format!(
                    "{name}: head geometry (headRadius/headSquashY/eyeSpacing/\
                     eyeScale/headLineWidth) — the head ring is project-scale \
                     in this model; no per-figure head geometry"
                ),
            );
        }
        if let Some(hand) = str_field(design_v, "handStyle") {
            push_unique(
                &mut report.unmapped,
                format!(
                    "{name}: handStyle '{hand}' — segment ends carry no hand \
                     profile in this model"
                ),
            );
        }
        let palette_keys = ["outlineColor", "eyeColor", "accent"];
        if palette_keys.iter().any(|k| design_v.get(k).is_some()) {
            push_unique(
                &mut report.unmapped,
                format!(
                    "{name}: outlineColor/eyeColor/accent — a figure carries \
                     one colour plus node overrides; ink/head/limb mapped, \
                     the rest has no home"
                ),
            );
        }
    }
    if let Some(voice) = str_field(bible, "voice") {
        push_unique(
            &mut report.unmapped,
            format!("{name}: voice '{voice}' — the editor has no TTS voices"),
        );
    }
    CastMember { id: id.to_string(), name, def, color }
}

/// The placeholder for a cast member with no Bible: the
/// classic humanoid, unchanged, named in the report.
fn placeholder_member(id: &str, report: &mut ImportReport) -> CastMember {
    report.missing_bibles.push(id.to_string());
    push_unique(
        &mut report.notes,
        format!(
            "cast member '{id}' has no Character Bible — imported as a \
             default humanoid figure; supply the Bible to get its design"
        ),
    );
    CastMember {
        id: id.to_string(),
        name: id.to_string(),
        def: FigureDef::humanoid(),
        color: 0xE8E8EC,
    }
}

// ----------------------------------------------------------------
// Story reading
// ----------------------------------------------------------------

/// One dialogue line as the story states it, before
/// placement: `who` is the raw cast id, `text` verbatim.
struct RawLine {
    who: Option<String>,
    text: String,
}

/// The dialogue lines of one beat, across the format
/// generations: v3+ `dialogue[]`, v2 `line` / `lines[]`,
/// and the compiler's accepted `say[]` alias.
fn beat_lines(beat: &Value) -> Vec<RawLine> {
    let mut lines = Vec::new();
    let mut take = |v: &Value| {
        if let Some(text) = str_field(v, "text") {
            lines.push(RawLine {
                who: str_field(v, "who").map(str::to_string),
                text: text.to_string(),
            });
        }
    };
    if let Some(dialogue) = beat.get("dialogue").and_then(Value::as_array) {
        for line in dialogue {
            take(line);
        }
    }
    if let Some(line) = beat.get("line") {
        if line.is_object() {
            take(line);
        }
    }
    for key in ["lines", "say"] {
        if let Some(list) = beat.get(key).and_then(Value::as_array) {
            for line in list {
                take(line);
            }
        }
    }
    lines
}

/// The cast ids a beat puts on screen, in order: the v3+
/// `actors` object keys, or the v2 per-actor top-level
/// keys (any beat key whose value is an actor spec).
fn beat_actor_ids(beat: &Value, cast_ids: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if let Some(actors) = beat.get("actors") {
        if let Some(map) = actors.as_object() {
            for key in map.keys() {
                if !ids.contains(key) {
                    ids.push(key.clone());
                }
            }
        } else if let Some(list) = actors.as_array() {
            for entry in list {
                if let Some(id) = entry.as_str() {
                    if !ids.iter().any(|i| i == id) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
    }
    if let Some(map) = beat.as_object() {
        for (key, value) in map {
            if cast_ids.contains(key)
                && value.is_object()
                && value.get("action").is_some()
                && !ids.contains(key)
            {
                ids.push(key.clone());
            }
        }
    }
    ids
}

/// The compiler's spoken-length estimate for one line, in
/// seconds (`compile.py`'s `line_seconds`).
fn line_seconds(text: &str) -> f64 {
    let words = text.split_whitespace().count().max(1) as f64;
    words / WORDS_PER_SECOND + LINE_SETTLE_S
}

// ----------------------------------------------------------------
// The import
// ----------------------------------------------------------------

/// Import a parsed story (with its cast's parsed Bibles)
/// into a project. Pure: no IO, no editor state. See the
/// module docs for the mapping and the honesty contract.
pub fn import_story(story: &Value, bibles: &[Value]) -> Result<ImportedStory, String> {
    let mut report = ImportReport::default();

    // Bibles by id. A document counts as a Bible when it
    // has both an `id` and a `design` block — the
    // compiler's own rule (`archetypes.json`, a preset
    // map, is data, not a Bible, and is skipped).
    let mut bible_by_id: Vec<(String, &Value)> = Vec::new();
    for bible in bibles {
        if let (Some(id), Some(_)) = (str_field(bible, "id"), bible.get("design")) {
            if bible.get("design").map(Value::is_object) == Some(true) {
                bible_by_id.push((id.to_string(), bible));
            }
        }
    }
    let bible_for = |id: &str| bible_by_id.iter().find(|(b, _)| b == id).map(|(_, v)| *v);

    // ---- Story head ------------------------------------
    let beats = story
        .get("beats")
        .and_then(Value::as_array)
        .filter(|b| !b.is_empty())
        .ok_or_else(|| "story has no beats — nothing to import".to_string())?;
    report.beats = beats.len();
    report.source_version = story.get("version").and_then(Value::as_i64);
    report.title = str_field(story, "title")
        .map(str::to_string)
        .unwrap_or_else(|| "Untitled".to_string());

    let story_fps = num_field(story, "fps")
        .filter(|f| f.is_finite() && *f >= 1.0 && *f <= 240.0)
        .unwrap_or(DEFAULT_STORY_FPS);
    report.fps = story_fps as f32;
    if story.get("fps").is_none() {
        report.notes.push(format!(
            "story carries no fps — beats converted at the harness \
             contract rate of {DEFAULT_STORY_FPS} fps"
        ));
    }

    // Stage: the story's resolution when it carries one
    // (the v3+ render contract), else the editor default.
    let mut stage = [960.0f32, 720.0f32];
    if let Some(res) = story.get("resolution") {
        if let (Some(w), Some(h)) = (num_field(res, "width"), num_field(res, "height")) {
            if w > 0.0 && h > 0.0 {
                stage = [w as f32, h as f32];
                report.notes.push(format!(
                    "stage set from the story's resolution ({w}×{h})"
                ));
            }
        }
    }
    let scale = (stage[1] as f64 / DESIGN_SPACE_HEIGHT) as f32;

    // ---- Cast ------------------------------------------
    // Story order first: the `characters[]` list when the
    // story carries one, then any id a beat or a line
    // references, in first-appearance order.
    let mut cast_ids: Vec<String> = Vec::new();
    if let Some(chars) = story.get("characters").and_then(Value::as_array) {
        for entry in chars {
            if let Some(id) = str_field(entry, "id") {
                add_cast_id(&mut cast_ids, id);
            }
        }
    }
    for beat in beats {
        for id in beat_actor_ids(beat, &cast_ids) {
            add_cast_id(&mut cast_ids, &id);
        }
        for line in beat_lines(beat) {
            if let Some(who) = &line.who {
                add_cast_id(&mut cast_ids, who);
            }
        }
    }
    // v2 stories name their cast only through the beat
    // keys; with no references at all, fall back to the
    // Bibles supplied (the v2 fixed cast rides along).
    if cast_ids.is_empty() {
        for (id, _) in &bible_by_id {
            add_cast_id(&mut cast_ids, id);
        }
    }

    let mut cast: Vec<CastMember> = Vec::new();
    for id in &cast_ids {
        let member = match bible_for(id) {
            Some(bible) => figure_from_bible(id, bible, scale, &mut report),
            None => placeholder_member(id, &mut report),
        };
        cast.push(member);
    }
    // Per-story appearance overrides (v3+ `characters[]`
    // entries carrying an `appearance` block) recolour the
    // Bible-built figure; the fields with no home are
    // reported like the Bible's own.
    if let Some(chars) = story.get("characters").and_then(Value::as_array) {
        for entry in chars {
            let (Some(id), Some(appearance)) =
                (str_field(entry, "id"), entry.get("appearance"))
            else {
                continue;
            };
            let Some(member) = cast.iter_mut().find(|m| m.id == id) else {
                continue;
            };
            if let Some(c) = str_field(appearance, "bodyColor").and_then(parse_hex_color) {
                member.color = c;
            }
            if let Some(c) = str_field(appearance, "headColor").and_then(parse_hex_color) {
                member.def.nodes[2].color = Some(c);
            }
            if let Some(c) = str_field(appearance, "limbColor").and_then(parse_hex_color) {
                for node in [3, 4, 5, 6, 7, 8, 9, 10] {
                    member.def.nodes[node].color = Some(c);
                }
            }
            for key in ["outlineColor", "eyeColor", "accentColor", "accessory"] {
                if appearance.get(key).is_some() {
                    push_unique(
                        &mut report.unmapped,
                        format!(
                            "{}: appearance override '{key}' — no model home \
                             (body/head/limb colours mapped)",
                            member.name
                        ),
                    );
                }
            }
        }
    }
    report.figures = cast.iter().map(|m| m.name.clone()).collect();
    // Bible prose is design documentation, not geometry —
    // one aggregate entry, not one per field per figure.
    let prose_bibles: Vec<&str> = cast
        .iter()
        .filter(|m| bible_for(&m.id).is_some())
        .map(|m| m.name.as_str())
        .collect();
    if !prose_bibles.is_empty() {
        push_unique(
            &mut report.unmapped,
            format!(
                "Character Bible prose for {} (personality, posture, mannerisms, \
                 signature poses/walk, costume, silhouette) — design documentation \
                 for the animator; it informs posing, it is not geometry",
                prose_bibles.join(", ")
            ),
        );
    }

    // ---- Beats → frames, chapters, cues -----------------
    let fps = story_fps;
    let mut beat_starts: Vec<usize> = Vec::with_capacity(beats.len());
    let mut cursor_s = 0.0f64;
    for beat in beats {
        beat_starts.push((cursor_s * fps).round() as usize);
        let seconds = num_field(beat, "seconds").unwrap_or(0.0);
        if seconds.is_finite() && seconds > 0.0 {
            cursor_s += seconds;
        }
    }
    let total_frames = (cursor_s * fps).round() as usize;
    if total_frames == 0 {
        return Err(
            "the story's beats total zero frames at its fps — nothing to import"
                .to_string(),
        );
    }
    report.frames = total_frames;
    let beat_end = |i: usize| -> usize {
        if i + 1 < beats.len() {
            beat_starts[i + 1]
        } else {
            total_frames
        }
    };

    // Chapters: a beat's `chapter` string opens a chapter
    // at that beat's first frame; it runs to the next
    // chapter beat or the end of the timeline.
    let mut chapter_marks: Vec<(usize, String)> = Vec::new();
    for (i, beat) in beats.iter().enumerate() {
        if let Some(title) = str_field(beat, "chapter") {
            let title = title.trim();
            if title.is_empty() {
                push_unique(
                    &mut report.notes,
                    format!("beat {}: chapter title is blank — no chapter opened", i + 1),
                );
            } else {
                chapter_marks.push((beat_starts[i], title.to_string()));
            }
        }
    }
    let mut chapters: Vec<Chapter> = Vec::new();
    for (k, (start, title)) in chapter_marks.iter().enumerate() {
        let end = chapter_marks
            .get(k + 1)
            .map(|(s, _)| *s)
            .unwrap_or(total_frames);
        if Chapter::validate(*start, end, title).is_ok() {
            chapters.push(Chapter {
                title: title.clone(),
                start_frame: *start,
                end_frame: end,
            });
        } else {
            push_unique(
                &mut report.notes,
                format!(
                    "chapter '{title}' covers an empty frame range — not added"
                ),
            );
        }
    }
    report.chapters = chapters.len();

    // Dialogue: every line a cue, verbatim, placed at the
    // compiler's spoken-length estimate inside its beat.
    let mut cues: Vec<SubtitleCue> = Vec::new();
    for (i, beat) in beats.iter().enumerate() {
        let lines = beat_lines(beat);
        let mut line_cursor_s = beat_starts[i] as f64 / fps;
        for (j, line) in lines.iter().enumerate() {
            if line.text.trim().is_empty() {
                push_unique(
                    &mut report.unmapped,
                    format!(
                        "beat {}, dialogue line {}: empty text — no cue created \
                         (there is no line to preserve)",
                        i + 1,
                        j + 1
                    ),
                );
                continue;
            }
            let need_s = line_seconds(&line.text);
            let start = (line_cursor_s * fps).floor() as usize;
            let end = ((line_cursor_s + need_s) * fps).ceil() as usize;
            let end = end.max(start + 1);
            if end > beat_end(i) {
                push_unique(
                    &mut report.notes,
                    format!(
                        "beat {}, line {} overruns its beat at the spoken-length \
                         estimate — kept verbatim at full length",
                        i + 1,
                        j + 1
                    ),
                );
            }
            // The speaker is the cast member's Bible
            // display name when the cast knows the id.
            let speaker_raw = line.who.as_deref().map(|who| {
                cast.iter()
                    .find(|m| m.id == who)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| who.to_string())
            });
            let speaker = match normalise_speaker(speaker_raw.as_deref()) {
                Ok(speaker) => speaker,
                Err(reason) => {
                    push_unique(
                        &mut report.notes,
                        format!(
                            "beat {}, line {}: speaker not credited ({reason}) — \
                             the line itself is imported verbatim, uncredited",
                            i + 1,
                            j + 1
                        ),
                    );
                    None
                }
            };
            cues.push(SubtitleCue {
                start_frame: start,
                end_frame: end,
                text: line.text.clone(),
                speaker,
            });
            line_cursor_s += need_s + LINE_PAD_S;
        }
    }
    report.cues = cues.len();

    // ---- The unmapped remainder, aggregated ------------
    let mut shot_beats = 0usize;
    let mut actor_specs = 0usize;
    let mut sfx_names: Vec<String> = Vec::new();
    let mut event_kinds: Vec<String> = Vec::new();
    let mut vfx_beats = 0usize;
    let mut note_beats = 0usize;
    let mut beat_sets: Vec<String> = Vec::new();
    let mut beat_looks: Vec<String> = Vec::new();
    for beat in beats {
        if beat.get("shot").is_some() {
            shot_beats += 1;
        }
        if let Some(actors) = beat.get("actors").and_then(Value::as_object) {
            actor_specs += actors.len();
        }
        if let Some(map) = beat.as_object() {
            for (key, value) in map {
                if cast_ids.contains(key) && value.is_object() {
                    actor_specs += 1;
                }
            }
        }
        if let Some(sfx) = str_field(beat, "sfx") {
            push_unique(&mut sfx_names, sfx.to_string());
        }
        if let Some(events) = beat.get("events").and_then(Value::as_array) {
            for event in events {
                if let Some(kind) = str_field(event, "type") {
                    push_unique(&mut event_kinds, kind.to_string());
                    if kind == "sfx" {
                        if let Some(name) = str_field(event, "name") {
                            push_unique(&mut sfx_names, name.to_string());
                        }
                    }
                }
            }
        }
        if beat.get("vfx").is_some() {
            vfx_beats += 1;
        }
        if beat.get("note").is_some() {
            note_beats += 1;
        }
        if let Some(set) = str_field(beat, "set") {
            push_unique(&mut beat_sets, set.to_string());
        }
        if let Some(look) = str_field(beat, "look") {
            push_unique(&mut beat_looks, look.to_string());
        }
    }
    if shot_beats > 0 {
        push_unique(
            &mut report.unmapped,
            format!(
                "shot (size/angle/lens/move/why) on {shot_beats} beat(s) — \
                 cinematography intent; this import authors no camera keys, \
                 the camera track is left empty rather than approximated"
            ),
        );
    }
    if actor_specs > 0 {
        push_unique(
            &mut report.unmapped,
            format!(
                "actor performance (action/expression/gaze/hold/handPose) in \
                 {actor_specs} actor spec(s) — no pose-track mapping yet; \
                 figures stand at rest on the imported timeline"
            ),
        );
    }
    if !sfx_names.is_empty() {
        push_unique(
            &mut report.unmapped,
            format!(
                "sfx ({}) — no audio clips are created by this import",
                sfx_names.join(", ")
            ),
        );
    }
    if !event_kinds.is_empty() {
        push_unique(
            &mut report.unmapped,
            format!(
                "beat events ({}) — prop states, tosses and effect events \
                 have no project equivalent yet",
                event_kinds.join(", ")
            ),
        );
    }
    if vfx_beats > 0 {
        push_unique(
            &mut report.unmapped,
            format!(
                "beat vfx on {vfx_beats} beat(s) — authored effects are not \
                 converted to particle events by this import"
            ),
        );
    }
    if note_beats > 0 {
        push_unique(
            &mut report.notes,
            format!(
                "{note_beats} beat(s) carry a human `note` — authoring \
                 annotation, not imported by design"
            ),
        );
    }
    // Story-level scenery and metadata.
    let story_set = story
        .get("environment")
        .and_then(|e| str_field(e, "type"))
        .or_else(|| str_field(story, "set"));
    if let Some(set) = story_set {
        push_unique(&mut beat_sets, set.to_string());
    }
    if !beat_sets.is_empty() {
        push_unique(
            &mut report.unmapped,
            format!(
                "set ({}) — scenery is not built by this import; the stage \
                 background is unchanged",
                beat_sets.join(", ")
            ),
        );
    }
    let story_look = story
        .get("environment")
        .and_then(|e| str_field(e, "look"))
        .or_else(|| str_field(story, "look"));
    if let Some(look) = story_look {
        push_unique(&mut beat_looks, look.to_string());
    }
    if !beat_looks.is_empty() {
        push_unique(
            &mut report.unmapped,
            format!(
                "look ({}) — lighting looks have no project equivalent",
                beat_looks.join(", ")
            ),
        );
    }
    if story.get("seed").is_some() {
        push_unique(
            &mut report.unmapped,
            "seed — the harness's deterministic random seed; the editor \
             has no seeded-random surface"
                .to_string(),
        );
    }
    if let Some(style) = story
        .get("artDirection")
        .and_then(|a| str_field(a, "characterStyle"))
    {
        push_unique(
            &mut report.unmapped,
            format!(
                "artDirection.characterStyle '{style}' — line profiles have \
                 no project equivalent"
            ),
        );
    }
    if story.get("durationSeconds").is_some() {
        push_unique(
            &mut report.notes,
            "durationSeconds is the story's declared total; the beats' own \
             seconds govern the imported timeline"
                .to_string(),
        );
    }

    // ---- Assemble the project ---------------------------
    let mut project = Project::new_demo();
    project.name = report.title.clone();
    project.fps = report.fps;
    project.stage = stage;
    project.camera = Vec::new();
    let ground_y = stage[1] * 0.78;
    let centre_x = stage[0] / 2.0;
    let count = cast.len();
    project.figures = cast
        .iter()
        .enumerate()
        .map(|(i, member)| FigureInstance {
            name: member.name.clone(),
            color: member.color,
            def: member.def.clone(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        })
        .collect();
    let roots: Vec<[f32; 2]> = cast
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let offset = (i as f32 - (count.saturating_sub(1)) as f32 / 2.0) * CAST_SPACING;
            [centre_x + offset, ground_y]
        })
        .collect();
    project.frames = (0..total_frames)
        .map(|_| Frame {
            poses: cast
                .iter()
                .zip(roots.iter())
                .map(|(member, root)| member.def.rest_pose(*root))
                .collect(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    project.subtitles = cues;
    project.chapters = chapters;

    Ok(ImportedStory { project, report })
}

/// The JSON-text entry point: parse the story and each
/// Bible document, then import. A Bible that does not
/// parse is named in `report.bible_errors` and its cast
/// member degrades to the placeholder path — one corrupt
/// Bible never sinks the whole import.
pub fn import_story_json(
    story_json: &str,
    bible_jsons: &[&str],
) -> Result<ImportedStory, String> {
    let story: Value = serde_json::from_str(story_json)
        .map_err(|e| format!("story is not valid JSON ({e})"))?;
    let mut bibles: Vec<Value> = Vec::new();
    let mut bible_errors: Vec<String> = Vec::new();
    for (i, raw) in bible_jsons.iter().enumerate() {
        match serde_json::from_str::<Value>(raw) {
            Ok(value) => bibles.push(value),
            Err(e) => bible_errors.push(format!("bible {}: {e}", i + 1)),
        }
    }
    let mut imported = import_story(&story, &bibles)?;
    imported.report.bible_errors = bible_errors;
    Ok(imported)
}

/// Import into the live editor core: the imported project
/// replaces the core's project as exactly ONE undo step
/// (the layered-import discipline), and the status line
/// reports the headline counts. Refusals change nothing
/// and spend nothing.
pub fn import_into_core(
    core: &mut EditorCore,
    story_json: &str,
    bible_jsons: &[&str],
) -> Result<ImportReport, String> {
    let imported = import_story_json(story_json, bible_jsons)?;
    core.snapshot_labeled("Import harness story");
    core.project = imported.project;
    core.current = 0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.status = format!(
        "Imported '{}': {} frames, {} figures, {} chapters, {} dialogue cues. \
         {} unmapped field group(s) — see the import report.",
        imported.report.title,
        imported.report.frames,
        imported.report.figures.len(),
        imported.report.chapters,
        imported.report.cues,
        imported.report.unmapped.len()
    );
    Ok(imported.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The Dad Bible's design block, verbatim values from
    /// `harness/characters/dad.json`.
    fn dad_bible() -> Value {
        json!({
            "id": "dad",
            "name": "Dad",
            "accessory": "cape",
            "voice": "local-flite-awb",
            "personality": "quiet, warm, sacrifices without ever naming it",
            "design": {
                "ink": "#232323", "headColor": "#FFFFFF",
                "limbColor": "#232323", "outlineColor": "#141414",
                "eyeColor": "#1A1A1A", "accent": "#8A5A2B",
                "headRadius": 52, "headSquashY": 0.86,
                "eyeSpacing": 34, "eyeScale": 0.95,
                "limbWidth": 20, "headLineWidth": 9,
                "handStyle": "round", "handScale": 1.05,
                "legEnd": "footless", "footLength": 56,
                "torso": 193, "neck": 28, "shoulderHalf": 65,
                "hipHalf": 27, "upperArm": 102, "foreArm": 93,
                "legUpper": 104, "legLower": 102
            }
        })
    }

    /// The child Son's Bible design, verbatim values from
    /// `harness/characters/sonkid.json` — much smaller
    /// proportions than Dad's, which the cast test leans
    /// on.
    fn sonkid_bible() -> Value {
        json!({
            "id": "sonkid",
            "name": "Son (child)",
            "accessory": "scarf",
            "design": {
                "ink": "#232323", "headColor": "#FFFFFF",
                "limbColor": "#232323", "outlineColor": "#141414",
                "eyeColor": "#1A1A1A", "accent": "#D42B1E",
                "headRadius": 71, "headSquashY": 1.03,
                "eyeSpacing": 40, "eyeScale": 1.3,
                "limbWidth": 12, "headLineWidth": 8,
                "handStyle": "open", "handScale": 0.9,
                "legEnd": "footless", "footLength": 44,
                "torso": 93, "neck": 20, "shoulderHalf": 36,
                "hipHalf": 16, "upperArm": 64, "foreArm": 52,
                "legUpper": 73, "legLower": 70
            }
        })
    }

    /// A condensed `story.last-promise.json` shape: v5,
    /// chapters on beats 1 and 3, narration + dialogue,
    /// the real lines of the episode's opening beats.
    fn last_promise_story() -> Value {
        json!({
            "version": 5,
            "id": "the-last-promise",
            "title": "THE LAST PROMISE — a father's sacrifice",
            "fps": 30,
            "resolution": {"width": 1080, "height": 1920},
            "environment": {"type": "park", "look": "golden"},
            "seed": 20261107,
            "characters": [{"id": "dad"}, {"id": "sonkid"}],
            "beats": [
                {
                    "seconds": 5,
                    "chapter": "Chapter One — A Poor Little Family",
                    "shot": {"size": "establishing", "angle": "eye",
                             "lens": "wide", "move": "push-in",
                             "why": "The little house at sunset"},
                    "actors": {
                        "dad": {"action": "walk", "expression": "exhausted", "hold": "box"},
                        "sonkid": {"action": "idle", "expression": "happy"}
                    },
                    "dialogue": [
                        {"who": "sonadult",
                         "text": "He had no money, and almost nothing at all."}
                    ]
                },
                {
                    "seconds": 8,
                    "actors": {
                        "dad": {"action": "talk", "expression": "determined"},
                        "sonkid": {"action": "idle", "expression": "curious"}
                    },
                    "dialogue": [
                        {"who": "dad",
                         "text": "One day, my son, you will achieve your dreams."},
                        {"who": "sonkid",
                         "text": "And I will make you proud, Dad!"}
                    ]
                },
                {
                    "seconds": 5,
                    "chapter": "Chapter Two — A Father's Sacrifice",
                    "actors": {
                        "dad": {"action": "idle", "expression": "exhausted"}
                    },
                    "dialogue": [
                        {"who": "sonadult",
                         "text": "His father worked until his body could barely move."}
                    ]
                }
            ]
        })
    }

    fn total_height(def: &FigureDef) -> f32 {
        // Hips→head-centre plus the longest leg chain: a
        // rough standing height for proportion checks.
        let up = def.nodes[1].length + def.nodes[2].length;
        let leg = def.nodes[7].length + def.nodes[8].length;
        up + leg
    }

    #[test]
    fn last_promise_shape_imports_end_to_end() {
        // sonadult narrates but has no Bible in this
        // fixture and no `characters[]` entry of his own
        // beyond the dialogue reference: he joins the cast
        // as a reported placeholder.
        let bibles = vec![dad_bible(), sonkid_bible()];
        let imported = import_story(&last_promise_story(), &bibles).unwrap();
        let (project, report) = (&imported.project, &imported.report);

        assert_eq!(project.name, "THE LAST PROMISE — a father's sacrifice");
        assert_eq!(project.fps, 30.0);
        assert_eq!(project.stage, [1080.0, 1920.0]);
        assert_eq!(report.beats, 3);
        // (5 + 8 + 5) s at 30 fps.
        assert_eq!(project.frames.len(), 540);
        assert_eq!(report.frames, 540);
        assert!(project.frames.iter().all(|f| f.poses.len() == 3));

        // Chapters: beat 1 opens chapter one over beats
        // 1–2 (frames 0..390), beat 3 opens chapter two.
        assert_eq!(project.chapters.len(), 2);
        assert_eq!(project.chapters[0].title, "Chapter One — A Poor Little Family");
        assert_eq!(
            (project.chapters[0].start_frame, project.chapters[0].end_frame),
            (0, 390)
        );
        assert_eq!(project.chapters[1].title, "Chapter Two — A Father's Sacrifice");
        assert_eq!(
            (project.chapters[1].start_frame, project.chapters[1].end_frame),
            (390, 540)
        );

        // Dialogue: every line a cue, verbatim, credited
        // with Bible display names.
        assert_eq!(report.cues, 4);
        let texts: Vec<&str> = project.subtitles.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "He had no money, and almost nothing at all.",
                "One day, my son, you will achieve your dreams.",
                "And I will make you proud, Dad!",
                "His father worked until his body could barely move.",
            ]
        );
        let speakers: Vec<Option<&str>> =
            project.subtitles.iter().map(|c| c.speaker.as_deref()).collect();
        assert_eq!(
            speakers,
            vec![Some("sonadult"), Some("Dad"), Some("Son (child)"), Some("sonadult")]
        );
        // Cue placement: line 1 starts its beat; the
        // two-line beat's cues are sequential, inside the
        // beat's span extended by nothing — both fit.
        assert_eq!(project.subtitles[0].start_frame, 0);
        let (b1, b2) = (&project.subtitles[1], &project.subtitles[2]);
        assert_eq!(b1.start_frame, 150);
        assert!(b2.start_frame >= b1.end_frame, "beat lines do not overlap");
        assert!(b2.end_frame <= 390, "both lines fit their beat");
        assert_eq!(project.subtitles[3].start_frame, 390);

        // Cast: story order, then the narrator discovered
        // from dialogue. Dad wears his Bible's ink; his
        // white head rides as the head node's colour.
        assert_eq!(report.figures, vec!["Dad", "Son (child)", "sonadult"]);
        assert_eq!(report.missing_bibles, vec!["sonadult"]);
        let dad = &project.figures[0];
        assert_eq!(dad.color, 0x232323);
        assert_eq!(dad.def.nodes[2].color, Some(0xFFFFFF));
        assert!(dad.def.nodes[2].head);
        assert_eq!(dad.def.nodes.len(), 11, "still a stickman");

        // The report names what did not map.
        let unmapped = report.unmapped.join("\n");
        assert!(unmapped.contains("shot"), "shots reported: {unmapped}");
        assert!(unmapped.contains("accessory 'cape'"), "accessory reported");
        assert!(unmapped.contains("set (park)"), "set reported");
        assert!(unmapped.contains("look (golden)"), "look reported");
        assert!(unmapped.contains("action/expression"), "performance reported");
    }

    #[test]
    fn bible_proportions_survive_the_rescale() {
        let bibles = vec![dad_bible(), sonkid_bible()];
        let imported = import_story(&last_promise_story(), &bibles).unwrap();
        let dad = &imported.project.figures[0].def;
        let kid = &imported.project.figures[1].def;
        // At scale 1.0 (the stage IS the design space):
        // Dad's chest = torso − neck, the kid is built
        // smaller in every shared segment.
        assert_eq!(dad.nodes[1].length, 165.0);
        assert_eq!(dad.nodes[2].length, 28.0);
        assert!(total_height(kid) < total_height(dad));
        assert!(kid.nodes[3].length < dad.nodes[3].length);
        // Bible stroke width rides along: Dad 20, kid 12.
        assert_eq!(dad.nodes[1].thickness, 20.0);
        assert_eq!(kid.nodes[1].thickness, 12.0);
    }

    #[test]
    fn v2_story_imports_with_per_actor_keys() {
        let story = json!({
            "version": 2,
            "title": "The Ultimate Move",
            "set": "dojo",
            "look": "golden",
            "seed": 20261108,
            "beats": [
                {
                    "seconds": 5,
                    "shot": {"size": "wide", "angle": "eye", "lens": "wide",
                             "move": "static", "why": "establish the dojo"},
                    "iq": {"action": "point", "expression": "confident", "gaze": "dot"},
                    "dot": {"action": "idle", "expression": "neutral", "gaze": "donut"},
                    "line": {"who": "iq", "text": "Today, I finally beat Dot."},
                    "sfx": "gong"
                },
                {
                    "seconds": 5,
                    "iq": {"action": "combatIdle", "expression": "suspicious"},
                    "dot": {"action": "point", "expression": "deadpan"},
                    "lines": [{"who": "dot", "text": "You have never hit me once."}]
                }
            ]
        });
        let iq = json!({
            "id": "iq", "name": "IQ", "accessory": "bowtie",
            "design": {"ink": "#2686E8", "headColor": "#2686E8",
                       "limbColor": "#2686E8", "torso": 204, "neck": 54,
                       "upperArm": 124, "foreArm": 118, "legUpper": 156,
                       "legLower": 146, "limbWidth": 20}
        });
        let dot = json!({
            "id": "dot", "name": "Dot", "accessory": "glasses",
            "design": {"ink": "#E8862E", "headColor": "#E8862E",
                       "limbColor": "#E8862E", "torso": 172, "neck": 40,
                       "upperArm": 106, "foreArm": 100, "legUpper": 138,
                       "legLower": 130, "limbWidth": 25}
        });
        let imported = import_story(&story, &[iq, dot]).unwrap();
        let (project, report) = (&imported.project, &imported.report);
        // No fps/resolution in a v2 story: 30 fps contract
        // rate, editor-default stage.
        assert_eq!(project.fps, 30.0);
        assert_eq!(project.stage, [960.0, 720.0]);
        assert_eq!(project.frames.len(), 300);
        assert_eq!(report.figures, vec!["IQ", "Dot"]);
        assert_eq!(project.figures[0].color, 0x2686E8);
        assert_eq!(project.figures[1].color, 0xE8862E);
        assert_eq!(project.subtitles.len(), 2);
        assert_eq!(project.subtitles[0].text, "Today, I finally beat Dot.");
        assert_eq!(project.subtitles[0].speaker.as_deref(), Some("IQ"));
        assert_eq!(project.subtitles[1].speaker.as_deref(), Some("Dot"));
        assert!(report.unmapped.join("\n").contains("sfx (gong)"));
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("no fps")));
    }

    #[test]
    fn an_overrunning_line_is_kept_verbatim_at_full_length() {
        // 17 words at 2.4 w/s ≈ 7.3 s of speech in a 2.5 s
        // beat: the cue keeps its spoken length and the
        // report says so — the line is never trimmed to
        // fit (verbatim is a hard rule).
        let story = json!({
            "version": 5,
            "title": "Overrun",
            "fps": 30,
            "characters": [{"id": "dad"}],
            "beats": [{
                "seconds": 2.5,
                "actors": {"dad": {"action": "talk"}},
                "dialogue": [
                    {"who": "dad",
                     "text": "This is a very long line that cannot possibly fit inside one short beat at all."},
                    {"who": "dad", "text": "Short."}
                ]
            }]
        });
        let imported = import_story(&story, &[dad_bible()]).unwrap();
        let cues = &imported.project.subtitles;
        assert_eq!(cues.len(), 2, "both lines imported");
        assert_eq!(
            cues[0].text,
            "This is a very long line that cannot possibly fit inside one short beat at all."
        );
        assert!(cues[0].end_frame > 75, "cue extends past the 75-frame beat");
        assert_eq!(cues[1].text, "Short.");
        assert!(imported
            .report
            .notes
            .iter()
            .any(|n| n.contains("overruns its beat")));
    }

    #[test]
    fn unknown_character_gets_a_reported_placeholder() {
        let story = json!({
            "version": 5,
            "title": "Ghost",
            "fps": 30,
            "characters": [{"id": "ghost"}],
            "beats": [{
                "seconds": 3,
                "actors": {"ghost": {"action": "idle"}},
                "dialogue": [{"who": "ghost", "text": "Boo."}]
            }]
        });
        let imported = import_story(&story, &[]).unwrap();
        assert_eq!(imported.report.missing_bibles, vec!["ghost"]);
        assert_eq!(imported.project.figures.len(), 1);
        assert_eq!(imported.project.figures[0].def.nodes.len(), 11);
        assert_eq!(imported.project.subtitles[0].text, "Boo.");
        assert_eq!(
            imported.project.subtitles[0].speaker.as_deref(),
            Some("ghost"),
            "an unknown speaker keeps their story id as the credit"
        );
    }

    #[test]
    fn appearance_overrides_recolour_the_bible_figure() {
        let story = json!({
            "version": 5,
            "title": "Recolour",
            "fps": 30,
            "characters": [{
                "id": "dad",
                "appearance": {"bodyColor": "#112233", "accentColor": "#FF0000"}
            }],
            "beats": [{"seconds": 3, "actors": {"dad": {"action": "idle"}}}]
        });
        let imported = import_story(&story, &[dad_bible()]).unwrap();
        assert_eq!(imported.project.figures[0].color, 0x112233);
        assert!(imported
            .report
            .unmapped
            .join("\n")
            .contains("appearance override 'accentColor'"));
    }

    #[test]
    fn malformed_stories_refuse() {
        assert!(import_story(&json!({}), &[]).is_err(), "no beats");
        assert!(
            import_story(&json!({"beats": []}), &[]).is_err(),
            "empty beats"
        );
        assert!(
            import_story(
                &json!({"beats": [{"actors": {}}]}),
                &[]
            )
            .is_err(),
            "beats with no seconds total zero frames"
        );
        assert!(import_story_json("{not json", &[]).is_err());
        let ok = import_story_json(
            &json!({"title": "T", "beats": [{"seconds": 2}]}).to_string(),
            &["{broken bible"],
        )
        .unwrap();
        assert_eq!(ok.report.bible_errors.len(), 1);
        assert_eq!(ok.project.frames.len(), 60, "2 s at the 30 fps default");
    }

    #[test]
    fn import_into_core_spends_exactly_one_undo_step() {
        let story = json!({
            "version": 5,
            "title": "Core",
            "fps": 30,
            "characters": [{"id": "dad"}],
            "beats": [{
                "seconds": 3,
                "chapter": "Chapter One — Start",
                "actors": {"dad": {"action": "idle"}},
                "dialogue": [{"who": "dad", "text": "Hello."}]
            }]
        });
        let mut core = EditorCore::new();
        let before = core.undo_depth();
        let report = import_into_core(
            &mut core,
            &story.to_string(),
            &[&dad_bible().to_string()],
        )
        .unwrap();
        assert_eq!(core.undo_depth(), before + 1);
        assert_eq!(core.undo_label(), Some("Import harness story"));
        assert_eq!(core.project.name, "Core");
        assert_eq!(core.project.chapters.len(), 1);
        assert_eq!(report.cues, 1);
        assert!(core.status.contains("Imported 'Core'"));
        // A refused import spends nothing.
        let before = core.undo_depth();
        assert!(import_into_core(&mut core, "{}", &[]).is_err());
        assert_eq!(core.undo_depth(), before);
    }
}
