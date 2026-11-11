//! Episode QA probe (Roadmap Wave 2, W2-20): the pure
//! measurement pass over a rendered episode file —
//! ffprobe/ffmpeg sampling, burn-in ink checks, chapter
//! and cue expectations, and the pass/fail verdict.
//! Moved here from the export panel (`rune_sec`,
//! bin-only) in the Wave 2 integration fix rounds so
//! the MCP `episode_qa_probe` tool (lib) can call it:
//! the probe never touches panel state, only a path
//! and a project.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};


/// What a finished export was supposed to be, read off
/// the project the job ran on: the duration and sampled
/// frame count of the exported window, whether its
/// audio was expected to carry the project's audible
/// clips, and — when a subtitle cue overlaps the window
/// — where the burn-in spot-check looks.
#[derive(Clone, Debug, PartialEq)]
pub struct QaExpectations {
    pub duration_secs: f32,
    pub frames: Option<usize>,
    pub audio_expected: bool,
    pub burn_in: Option<QaBurnIn>,
}

/// The burn-in spot-check's target: the frame time
/// (seconds into the exported file) of the middle of
/// the longest-overlapping cue, and the style's text
/// colour and anchor the ink count reads.
#[derive(Clone, Debug, PartialEq)]
pub struct QaBurnIn {
    pub at_secs: f32,
    pub text_rgb: [u8; 3],
    pub position: crate::subtitles::SubtitlePosition,
}

/// Build the expectations for one finished video
/// export. `window` is the job's frame range (`None` =
/// the full timeline); `audio_possible` is false only
/// for the one path that never muxes (the full WebM),
/// so an absent audio stream there is the documented
/// shape, not a loss. Pure apart from the clip-file
/// stats (the `range_audio_note_shown` audibility rule:
/// unmuted and the file present).
pub fn qa_expectations(
    project: &crate::model::Project,
    window: Option<(usize, usize)>,
    audio_possible: bool,
) -> QaExpectations {
    let (start_frame, end_frame) = match window {
        Some((a, b)) => (a, b + 1),
        None => (0, project.frames.len()),
    };
    let start_secs = project.seconds_at_frame_time(start_frame as f32);
    let duration_secs = match window {
        Some(_) => project.seconds_at_frame_time(end_frame as f32) - start_secs,
        None => project.total_seconds(),
    };
    let frames = match window {
        Some(_) => Some(
            (project.beats_at_frame_time(end_frame as f32)
                - project.beats_at_frame_time(start_frame as f32))
            .round()
            .max(0.0) as usize,
        ),
        None => Some(crate::export::sampled_frame_total(project)),
    };
    // A clip sounds in the window when it is unmuted,
    // its file exists, and it starts before the window
    // ends (a clip's own length is the mux plan's
    // business; a clip starting earlier may still be
    // sounding inside the window).
    let audio_expected = audio_possible
        && project.audio.iter().any(|clip| {
            !clip.muted && clip.start_frame < end_frame && Path::new(&clip.path).exists()
        });
    // The cue with the longest overlap of the window is
    // the spot-check: its midpoint is the frame most
    // certainly carrying its text.
    let mut burn_in: Option<(usize, usize, usize)> = None; // (overlap, mid frame, cue idx)
    for (i, cue) in project.subtitles.iter().enumerate() {
        let lo = cue.start_frame.max(start_frame);
        let hi = cue.end_frame.min(end_frame);
        if hi > lo {
            let overlap = hi - lo;
            if burn_in.map_or(true, |(best, _, _)| overlap > best) {
                burn_in = Some((overlap, (lo + hi) / 2, i));
            }
        }
    }
    let burn_in = burn_in.map(|(_, mid, _i)| {
        let rgb = project.subtitle_style.text_color;
        QaBurnIn {
            at_secs: project.seconds_at_frame_time(mid as f32) - start_secs,
            text_rgb: [
                ((rgb >> 16) & 0xFF) as u8,
                ((rgb >> 8) & 0xFF) as u8,
                (rgb & 0xFF) as u8,
            ],
            position: project.subtitle_style.position,
        }
    });
    QaExpectations {
        duration_secs,
        frames,
        audio_expected,
        burn_in,
    }
}

/// What `ffprobe` says about a produced file: container
/// duration, stream presence, the video stream's frame
/// count when the container carries one (else derived
/// from the average frame rate), and its dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoProbe {
    pub duration_secs: f32,
    pub has_video: bool,
    pub has_audio: bool,
    pub frames: Option<u64>,
    pub width: u32,
    pub height: u32,
}

/// Parse `ffprobe -show_format -show_streams -of json`
/// output. Pure, so the parser is pinnable without the
/// binary. `None` on unparseable output or a missing
/// duration — a probe that cannot read the file says
/// so by absence, and the verdict treats absence as
/// failure.
pub fn parse_ffprobe_json(text: &str) -> Option<VideoProbe> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let duration_secs = value
        .get("format")?
        .get("duration")?
        .as_str()?
        .parse::<f32>()
        .ok()?;
    let mut probe = VideoProbe {
        duration_secs,
        has_video: false,
        has_audio: false,
        frames: None,
        width: 0,
        height: 0,
    };
    if let Some(streams) = value.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            match stream.get("codec_type").and_then(|t| t.as_str()) {
                Some("video") => {
                    probe.has_video = true;
                    probe.width =
                        stream.get("width").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
                    probe.height =
                        stream.get("height").and_then(|h| h.as_u64()).unwrap_or(0) as u32;
                    probe.frames = stream
                        .get("nb_frames")
                        .and_then(|n| n.as_str())
                        .and_then(|n| n.parse::<u64>().ok())
                        .or_else(|| {
                            let rate = stream.get("avg_frame_rate")?.as_str()?;
                            let (num, den) = rate.split_once('/')?;
                            let fps = num.parse::<f32>().ok()? / den.parse::<f32>().ok()?;
                            if fps > 0.0 {
                                Some((fps * duration_secs).round() as u64)
                            } else {
                                None
                            }
                        });
                }
                Some("audio") => probe.has_audio = true,
                _ => {}
            }
        }
    }
    Some(probe)
}

/// Probe a produced file with the system `ffprobe` —
/// the same external tool the exporters' own duration
/// probe uses. `None` when ffprobe is missing, fails,
/// or its output does not parse.
pub fn probe_video_file(path: &Path) -> Option<VideoProbe> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ffprobe_json(&String::from_utf8_lossy(&output.stdout))
}

/// Peak and RMS amplitude of interleaved samples — the
/// metering half of RM1's family, over the decoded
/// export rather than the mix. `(0.0, 0.0)` for no
/// samples.
pub fn audio_stats(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut peak = 0.0f32;
    let mut squares = 0.0f64;
    for s in samples {
        peak = peak.max(s.abs());
        squares += (*s as f64) * (*s as f64);
    }
    (peak, (squares / samples.len() as f64).sqrt() as f32)
}

/// An amplitude as a dBFS reading with one decimal;
/// silence (and below) reads "-inf dB".
pub fn fmt_db(v: f32) -> String {
    if v <= 0.0 {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", 20.0 * v.log10())
    }
}

/// A unique scratch path for one probe decode.
fn qa_temp_path(tag: &str, ext: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stykanimate-qa-{}-{n}-{tag}.{ext}",
        std::process::id()
    ))
}

/// Count pixels within tolerance of `colour` in the
/// anchored band (top or bottom quarter) of an RGB
/// frame — the burn-in spot-check's ink counter. Pure.
pub fn band_ink_count(rgb: &[u8], width: usize, height: usize, top: bool, colour: [u8; 3]) -> usize {
    if width == 0 || height == 0 || rgb.len() < width * height * 3 {
        return 0;
    }
    let band = (height / 4).max(1);
    let rows = if top { 0..band } else { height - band..height };
    let mut count = 0;
    for y in rows {
        for x in 0..width {
            let i = (y * width + x) * 3;
            let d = (rgb[i] as i32 - colour[0] as i32).abs()
                + (rgb[i + 1] as i32 - colour[1] as i32).abs()
                + (rgb[i + 2] as i32 - colour[2] as i32).abs();
            if d <= 72 {
                count += 1;
            }
        }
    }
    count
}

/// The burn-in spot-check measurement: decode the
/// cue-midpoint frame at 480 pt wide and count
/// text-colour ink in the anchored band. `None` when
/// the frame does not decode.
pub fn burn_in_ink(path: &Path, burn: &QaBurnIn, aspect: (u32, u32)) -> Option<usize> {
    if aspect.0 == 0 || aspect.1 == 0 {
        return None;
    }
    let width = 480usize;
    let height = ((width as f32 * aspect.1 as f32 / aspect.0 as f32) / 2.0).round() as usize * 2;
    if height == 0 {
        return None;
    }
    let tmp = qa_temp_path("burnin", "rgb");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-ss"])
        .arg(format!("{:.3}", burn.at_secs))
        .arg("-i")
        .arg(path)
        .args(["-frames:v", "1", "-vf"])
        .arg(format!("scale={width}:{height}"))
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24"])
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
    Some(band_ink_count(
        &bytes,
        width,
        height,
        matches!(burn.position, crate::subtitles::SubtitlePosition::Top),
        burn.text_rgb,
    ))
}

/// Cap on sampled frames per episode scan — past it
/// the effective interval widens so a long episode's
/// probe stays bounded, and the widened interval is
/// what the stretch maths uses.
pub const EPISODE_FRAME_SAMPLE_MAX: usize = 240;

/// How many cues the subtitle spot-check decodes ink
/// for at most; more cues are sampled evenly across
/// the episode and the report says how many of how
/// many were checked.
pub const EPISODE_CUE_SPOT_MAX: usize = 12;

/// The audio scan's metering window: peak per 50 ms
/// window decides whether that window is silent.
const EPISODE_AUDIO_WINDOW_SECS: f32 = 0.05;

/// Every threshold the episode verdict applies, with
/// the defaults the constants above state. A caller
/// (a future panel surface, an MCP tool) may pass its
/// own; the verdict names the values it applied in the
/// evidence it returns.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeQaParams {
    /// Peak at or below this counts as silence (the
    /// RM29 constant by default).
    pub silence_peak: f32,
    /// Silent stretches at or above this fail.
    pub silence_min_secs: f32,
    /// Mean luma at or below this counts as black (the
    /// RM29 constant by default).
    pub black_luma: f32,
    /// Black stretches at or above this fail.
    pub black_min_secs: f32,
    /// Frozen stretches at or above this fail.
    pub frozen_min_secs: f32,
    /// Requested spacing of the frame scan.
    pub frame_interval_secs: f32,
    /// Chapter-start tolerance.
    pub chapter_tol_secs: f32,
    /// Reading-speed reporting reference — never a
    /// verdict (D-9).
    pub cps_reference: f32,
    /// Ink pixels a spot-check must find (the RM29
    /// constant by default).
    pub ink_min: usize,
}

impl Default for EpisodeQaParams {
    fn default() -> Self {
        EpisodeQaParams {
            silence_peak: QA_SILENCE_PEAK,
            silence_min_secs: EPISODE_SILENCE_MIN_SECS,
            black_luma: QA_BLACK_LUMA,
            black_min_secs: EPISODE_BLACK_MIN_SECS,
            frozen_min_secs: EPISODE_FROZEN_MIN_SECS,
            frame_interval_secs: EPISODE_FRAME_INTERVAL_SECS,
            chapter_tol_secs: EPISODE_CHAPTER_TOL_SECS,
            cps_reference: EPISODE_CPS_REFERENCE,
            ink_min: QA_INK_MIN,
        }
    }
}

/// One chapter marker the export promised, resolved
/// from the very FFMETADATA document W2-18 embeds
/// (`export::ffmetadata_chapters`), so the probe checks
/// the file against the exporter's own resolution —
/// first-match shadowing, gap handling and the final
/// chapter's run to the episode end included, never a
/// second resolution that could drift from it.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeChapterExpectation {
    pub title: String,
    pub start_secs: f32,
    pub end_secs: f32,
}

/// One chapter title card the episode should show: the
/// chapter (0-based, for the report), its verbatim
/// title, and the ink spot-check target — the middle of
/// the card's window (`title_cards::title_card_window`,
/// clamped to the timeline) in the subtitle style's
/// text colour.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeCardExpectation {
    pub chapter: usize,
    pub title: String,
    pub at_secs: f32,
    pub text_rgb: [u8; 3],
}

/// The project-side facts about one subtitle cue the
/// episode verdict judges or reports: its authored
/// range, its character count, and its reading speed
/// on the shared duration-weighted clock (RM7) —
/// `chars / seconds`, 0 when the range is empty (the
/// range itself is the timing check's business).
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeCueFact {
    pub index: usize,
    pub start_frame: usize,
    pub end_frame: usize,
    pub chars: usize,
    pub cps: f32,
}

/// What a finished EPISODE export was supposed to be:
/// RM29's full-window expectations plus the episode
/// layer's own — chapter markers, title cards, and the
/// cue facts. `cue_targets` indexes `cue_facts`: the
/// cues the subtitle spot-check decodes ink for.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeExpectations {
    pub qa: QaExpectations,
    pub fps: f32,
    pub total_frames: usize,
    pub chapters: Vec<EpisodeChapterExpectation>,
    pub title_cards_enabled: bool,
    pub cards: Vec<EpisodeCardExpectation>,
    pub cue_facts: Vec<EpisodeCueFact>,
    pub cue_targets: Vec<usize>,
}

/// Parse the FFMETADATA document `ffmetadata_chapters`
/// produces back into chapter expectations. Pure.
/// Titles come back unescaped (the format's backslash
/// rule reversed), so they compare verbatim against
/// both the project and the file. A document with no
/// chapter blocks parses to an empty list.
pub fn parse_ffmetadata_chapters(doc: &str) -> Vec<EpisodeChapterExpectation> {
    fn unescape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        let mut chars = value.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
    let mut chapters = Vec::new();
    let mut current: Option<(u64, u64, String)> = None;
    for line in doc.lines() {
        let line = line.trim();
        if line == "[CHAPTER]" {
            if let Some((start, end, title)) = current.take() {
                chapters.push(EpisodeChapterExpectation {
                    title,
                    start_secs: start as f32 / 1000.0,
                    end_secs: end as f32 / 1000.0,
                });
            }
            current = Some((0, 0, String::new()));
        } else if let Some((start, end, title)) = &mut current {
            if let Some(v) = line.strip_prefix("START=") {
                *start = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("END=") {
                *end = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("title=") {
                *title = unescape(v);
            }
        }
    }
    if let Some((start, end, title)) = current.take() {
        chapters.push(EpisodeChapterExpectation {
            title,
            start_secs: start as f32 / 1000.0,
            end_secs: end as f32 / 1000.0,
        });
    }
    chapters
}

/// Build the episode expectations for one finished
/// full-timeline export of `project`. Pure apart from
/// the clip-file stats `qa_expectations` already reads.
pub fn episode_expectations(
    project: &crate::model::Project,
) -> EpisodeExpectations {
    let qa = qa_expectations(project, None, true);
    let total_frames = project.frames.len();
    let chapters = crate::export::ffmetadata_chapters(project)
        .map(|doc| parse_ffmetadata_chapters(&doc))
        .unwrap_or_default();
    // Title cards: one spot-check per chapter whose
    // card window falls inside the timeline, aimed at
    // the window's middle frame — the frame the card's
    // own alpha rule shows at full strength.
    let style = project.subtitle_style.sanitised();
    let text_rgb = [
        ((style.text_color >> 16) & 0xFF) as u8,
        ((style.text_color >> 8) & 0xFF) as u8,
        (style.text_color & 0xFF) as u8,
    ];
    let mut cards = Vec::new();
    if project.title_cards.enabled {
        for index in 0..project.chapters.len() {
            let Some((start, end)) =
                crate::title_cards::title_card_window(project, index)
            else {
                continue;
            };
            let end = end.min(total_frames);
            if end <= start || start >= total_frames {
                continue;
            }
            cards.push(EpisodeCardExpectation {
                chapter: index,
                title: project.chapters[index].title.clone(),
                at_secs: project.seconds_at_frame_time(((start + end) / 2) as f32),
                text_rgb,
            });
        }
    }
    // Cue facts for every cue; the spot-check targets
    // are all of them up to the cap, sampled evenly
    // beyond it, and only cues whose midpoint frame
    // falls inside the timeline (a cue past the end is
    // the timing check's finding, not an ink target).
    let cue_facts: Vec<EpisodeCueFact> = project
        .subtitles
        .iter()
        .enumerate()
        .map(|(index, cue)| {
            let secs = project.seconds_at_frame_time(cue.end_frame as f32)
                - project.seconds_at_frame_time(cue.start_frame as f32);
            let chars = cue.text.chars().count();
            EpisodeCueFact {
                index,
                start_frame: cue.start_frame,
                end_frame: cue.end_frame,
                chars,
                cps: if secs > 0.0 { chars as f32 / secs } else { 0.0 },
            }
        })
        .collect();
    let in_timeline: Vec<usize> = cue_facts
        .iter()
        .filter(|f| f.end_frame > f.start_frame && (f.start_frame + f.end_frame) / 2 < total_frames)
        .map(|f| f.index)
        .collect();
    let cue_targets = if in_timeline.len() <= EPISODE_CUE_SPOT_MAX {
        in_timeline
    } else {
        let mut picked: Vec<usize> = (0..EPISODE_CUE_SPOT_MAX)
            .map(|i| {
                in_timeline
                    [((i * (in_timeline.len() - 1)) as f32 / (EPISODE_CUE_SPOT_MAX - 1) as f32).round() as usize]
            })
            .collect();
        picked.dedup();
        picked
    };
    EpisodeExpectations {
        qa,
        fps: project.fps,
        total_frames,
        chapters,
        title_cards_enabled: project.title_cards.enabled,
        cards,
        cue_facts,
        cue_targets,
    }
}

/// One chapter marker as the produced file carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeFileChapter {
    pub title: String,
    pub start_secs: f32,
    pub end_secs: f32,
}

/// Parse `ffprobe -show_chapters -of json` output.
/// Pure. `None` on unparseable output (the caller reads
/// absence as Unmeasured, not as "no chapters" — a
/// chapterless file parses to an empty list).
pub fn parse_ffprobe_chapters_json(text: &str) -> Option<Vec<EpisodeFileChapter>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut chapters = Vec::new();
    if let Some(list) = value.get("chapters").and_then(|c| c.as_array()) {
        for chapter in list {
            let secs = |key: &str| {
                chapter
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(0.0)
            };
            chapters.push(EpisodeFileChapter {
                title: chapter
                    .get("tags")
                    .and_then(|t| t.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                start_secs: secs("start_time"),
                end_secs: secs("end_time"),
            });
        }
    }
    Some(chapters)
}

/// Read a produced file's chapter track with the
/// system `ffprobe`. `None` when ffprobe is missing,
/// fails, or its output does not parse.
pub fn probe_file_chapters(path: &Path) -> Option<Vec<EpisodeFileChapter>> {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_chapters"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ffprobe_chapters_json(&String::from_utf8_lossy(&output.stdout))
}

/// The episode audio scan: the whole-file peak and RMS
/// (RM29's metering, `audio_stats`) plus every silent
/// stretch — maximal runs of metering windows whose
/// peak sits at or below the silence line, as
/// `(start_secs, end_secs)`.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeAudioScan {
    pub peak: f32,
    pub rms: f32,
    pub silence_stretches: Vec<(f32, f32)>,
}

/// Scan decoded mono samples for level and silent
/// stretches. Pure, so the stretch maths is pinnable
/// without a file. Window boundaries quantise stretch
/// edges to the metering window (50 ms).
pub fn audio_scan(samples: &[f32], sample_rate: u32, silence_peak: f32) -> EpisodeAudioScan {
    let (peak, rms) = audio_stats(samples);
    let mut stretches = Vec::new();
    if sample_rate > 0 && !samples.is_empty() {
        let window = ((sample_rate as f32 * EPISODE_AUDIO_WINDOW_SECS).round() as usize).max(1);
        let window_secs = window as f32 / sample_rate as f32;
        let mut run_start: Option<usize> = None;
        let windows = samples.chunks(window).len();
        for (i, chunk) in samples.chunks(window).enumerate() {
            let silent = chunk.iter().all(|s| s.abs() <= silence_peak);
            if silent && run_start.is_none() {
                run_start = Some(i);
            } else if !silent {
                if let Some(start) = run_start.take() {
                    stretches.push((start as f32 * window_secs, i as f32 * window_secs));
                }
            }
        }
        if let Some(start) = run_start {
            stretches.push((start as f32 * window_secs, windows as f32 * window_secs));
        }
    }
    EpisodeAudioScan {
        peak,
        rms,
        silence_stretches: stretches,
    }
}

/// Decode a produced file's audio to mono samples with
/// the system `ffmpeg` — the same decode
/// `decode_audio_stats` meters, kept whole for the
/// stretch scan. `None` when there is nothing
/// decodable.
pub fn decode_audio_samples(path: &Path) -> Option<(Vec<f32>, u32)> {
    let tmp = qa_temp_path("epaudio", "wav");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", "44100"])
        .arg(&tmp)
        .status()
        .ok()?;
    let samples = if status.success() {
        crate::audio_edit::read_wav(&tmp.to_string_lossy())
            .ok()
            .map(|wav| (wav.samples, wav.sample_rate))
    } else {
        None
    };
    let _ = std::fs::remove_file(&tmp);
    samples
}

/// FNV-1a over a decoded gray frame — the frozen
/// scan's fingerprint. Two samples of an unchanging
/// picture hash alike; any visible change rehashes.
fn frame_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// One sampled frame of the episode scan: when it was
/// taken (the middle of the interval it stands for),
/// its mean luma, and its fingerprint.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeFrameSample {
    pub at_secs: f32,
    pub luma: f32,
    pub fingerprint: u64,
}

/// Decode one frame of a produced file small and gray
/// and measure it — one episode-scan sample, in
/// `frame_luma_at`'s decode shape plus the fingerprint.
fn episode_frame_sample_at(path: &Path, at_secs: f32) -> Option<EpisodeFrameSample> {
    let tmp = qa_temp_path("epframe", "gray");
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
    Some(EpisodeFrameSample {
        at_secs,
        luma: sum as f32 / bytes.len() as f32 / 255.0,
        fingerprint: frame_fingerprint(&bytes),
    })
}

/// Sample frames across a produced file at the
/// requested interval (widened past the sample cap),
/// returning the samples and the effective interval
/// the stretch maths must use. Undecodable samples are
/// skipped, as in RM29's black scan.
pub fn episode_frame_samples(
    path: &Path,
    duration_secs: f32,
    interval_secs: f32,
) -> (Vec<EpisodeFrameSample>, f32) {
    if duration_secs <= 0.0 || interval_secs <= 0.0 {
        return (Vec::new(), interval_secs);
    }
    let wanted = (duration_secs / interval_secs).ceil() as usize;
    let count = wanted.clamp(1, EPISODE_FRAME_SAMPLE_MAX);
    let interval = duration_secs / count as f32;
    let mut out = Vec::new();
    for i in 0..count {
        let t = interval * (i as f32 + 0.5);
        if let Some(sample) = episode_frame_sample_at(path, t) {
            out.push(sample);
        }
    }
    (out, interval)
}

/// The title-card spot-check measurement: decode the
/// card-window midpoint frame at 480 pt wide and count
/// text-colour ink in the centre band, in
/// `burn_in_ink`'s decode shape. `None` when the frame
/// does not decode.
pub fn card_ink_at(
    path: &Path,
    at_secs: f32,
    text_rgb: [u8; 3],
    aspect: (u32, u32),
) -> Option<usize> {
    if aspect.0 == 0 || aspect.1 == 0 {
        return None;
    }
    let width = 480usize;
    let height = ((width as f32 * aspect.1 as f32 / aspect.0 as f32) / 2.0).round() as usize * 2;
    if height == 0 {
        return None;
    }
    let tmp = qa_temp_path("epcard", "rgb");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-ss"])
        .arg(format!("{at_secs:.3}"))
        .arg("-i")
        .arg(path)
        .args(["-frames:v", "1", "-vf"])
        .arg(format!("scale={width}:{height}"))
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24"])
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
    Some(centre_ink_count(&bytes, width, height, text_rgb))
}

/// Everything the episode probe measured about one
/// file, before the verdict: the verdict is a pure
/// function of this plus the expectations and params,
/// so the whole decision matrix needs no files.
/// `cue_ink` / `card_ink` run parallel to the
/// expectations' `cue_targets` / `cards`.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeMeasurements {
    pub probe: Option<VideoProbe>,
    pub audio_scan: Option<EpisodeAudioScan>,
    pub frame_samples: Vec<EpisodeFrameSample>,
    pub frame_interval_secs: f32,
    pub file_chapters: Option<Vec<EpisodeFileChapter>>,
    pub cue_ink: Vec<Option<usize>>,
    pub card_ink: Vec<Option<usize>>,
}

impl Default for EpisodeMeasurements {
    fn default() -> Self {
        EpisodeMeasurements {
            probe: None,
            audio_scan: None,
            frame_samples: Vec::new(),
            frame_interval_secs: EPISODE_FRAME_INTERVAL_SECS,
            file_chapters: None,
            cue_ink: Vec::new(),
            card_ink: Vec::new(),
        }
    }
}

/// One named episode check's outcome. `Unmeasured` is
/// first-class: the check could not measure, which the
/// report states instead of passing silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpisodeCheckStatus {
    Pass,
    Fail,
    Unmeasured,
}

/// One named episode check: what it asked, how it
/// answered, and the evidence — measured values, the
/// parameter values applied, and timestamps for every
/// stretch it names.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeCheck {
    pub name: &'static str,
    pub status: EpisodeCheckStatus,
    pub evidence: String,
}

/// One probed episode's report: the named checks, the
/// roll-up (failed iff any check failed — Unmeasured
/// never fails and never passes a check, it is listed),
/// and a counted summary line of detector data.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeQaReport {
    pub dest: String,
    pub checks: Vec<EpisodeCheck>,
    pub passed: bool,
    pub summary: String,
}

fn episode_check(
    name: &'static str,
    status: EpisodeCheckStatus,
    evidence: String,
) -> EpisodeCheck {
    EpisodeCheck {
        name,
        status,
        evidence,
    }
}

/// Maximal runs of consecutive frame samples matching
/// `pred`, as `(start_secs, end_secs)` spans — each
/// sample stands for the interval centred on it.
fn sample_runs(
    samples: &[EpisodeFrameSample],
    interval: f32,
    pred: impl Fn(&EpisodeFrameSample) -> bool,
) -> Vec<(f32, f32)> {
    let mut runs = Vec::new();
    let mut start: Option<f32> = None;
    let mut last: f32 = 0.0;
    for sample in samples {
        if pred(sample) {
            if start.is_none() {
                start = Some(sample.at_secs - interval / 2.0);
            }
            last = sample.at_secs + interval / 2.0;
        } else if let Some(s) = start.take() {
            runs.push((s, last));
        }
    }
    if let Some(s) = start {
        runs.push((s, last));
    }
    runs
}

/// The episode verdict — pure. Every check names
/// itself, states its evidence, and applies only the
/// thresholds `params` carries; reading speed is
/// reported against its reference and judged by
/// nothing (D-9).
pub fn episode_qa_verdict(
    dest: &str,
    expect: &EpisodeExpectations,
    m: &EpisodeMeasurements,
    params: &EpisodeQaParams,
) -> EpisodeQaReport {
    let mut checks: Vec<EpisodeCheck> = Vec::new();
    // The cue-timing check is project-side: it judges
    // even when the file cannot be read at all.
    let timing = episode_cue_timing_check(expect, params);
    let Some(probe) = &m.probe else {
        checks.push(episode_check(
            "probe",
            EpisodeCheckStatus::Fail,
            "the produced file could not be probed".to_string(),
        ));
        for name in [
            "duration",
            "frame-count",
            "audio",
            "silence-stretches",
            "black-stretches",
            "frozen-stretches",
            "subtitles",
            "title-cards",
            "chapters",
        ] {
            checks.push(episode_check(
                name,
                EpisodeCheckStatus::Unmeasured,
                "the produced file could not be probed".to_string(),
            ));
        }
        checks.push(timing);
        return episode_report(dest, checks);
    };

    // Duration and frame count: RM29's tolerances, as
    // named episode checks.
    let duration_tol = (expect.qa.duration_secs * 0.15).max(0.75);
    if (probe.duration_secs - expect.qa.duration_secs).abs() > duration_tol {
        checks.push(episode_check(
            "duration",
            EpisodeCheckStatus::Fail,
            format!(
                "file runs {:.1}s, the project is {:.1}s ({} frames at {:.0} fps)",
                probe.duration_secs, expect.qa.duration_secs, expect.total_frames, expect.fps
            ),
        ));
    } else {
        checks.push(episode_check(
            "duration",
            EpisodeCheckStatus::Pass,
            format!(
                "file {:.1}s vs project {:.1}s ({} frames at {:.0} fps)",
                probe.duration_secs, expect.qa.duration_secs, expect.total_frames, expect.fps
            ),
        ));
    }
    match (probe.frames, expect.qa.frames) {
        (Some(got), Some(want)) => {
            let tol = ((want as f32 * 0.05).ceil() as u64).max(2);
            if got.abs_diff(want as u64) > tol {
                checks.push(episode_check(
                    "frame-count",
                    EpisodeCheckStatus::Fail,
                    format!("file holds {got} frames, the project samples {want}"),
                ));
            } else {
                checks.push(episode_check(
                    "frame-count",
                    EpisodeCheckStatus::Pass,
                    format!("{got} frames vs {want} sampled from the project"),
                ));
            }
        }
        _ => checks.push(episode_check(
            "frame-count",
            EpisodeCheckStatus::Unmeasured,
            "the container carries no frame count and no rate to derive one".to_string(),
        )),
    }

    // Audio level: present, expected, and not silent.
    if !probe.has_audio {
        if expect.qa.audio_expected {
            checks.push(episode_check(
                "audio",
                EpisodeCheckStatus::Fail,
                "audio stream missing though the project expects audio".to_string(),
            ));
        } else {
            checks.push(episode_check(
                "audio",
                EpisodeCheckStatus::Pass,
                "no audio stream; none expected".to_string(),
            ));
        }
    } else {
        match &m.audio_scan {
            None => {
                if expect.qa.audio_expected {
                    checks.push(episode_check(
                        "audio",
                        EpisodeCheckStatus::Fail,
                        "audio could not be decoded".to_string(),
                    ));
                } else {
                    checks.push(episode_check(
                        "audio",
                        EpisodeCheckStatus::Unmeasured,
                        "an audio stream is present but could not be decoded; none was expected"
                            .to_string(),
                    ));
                }
            }
            Some(scan) => {
                if expect.qa.audio_expected && scan.peak <= params.silence_peak {
                    checks.push(episode_check(
                        "audio",
                        EpisodeCheckStatus::Fail,
                        format!("audio is silent (peak {})", fmt_db(scan.peak)),
                    ));
                } else {
                    checks.push(episode_check(
                        "audio",
                        EpisodeCheckStatus::Pass,
                        format!(
                            "peak {} · RMS {}{}",
                            fmt_db(scan.peak),
                            fmt_db(scan.rms),
                            if expect.qa.audio_expected {
                                ""
                            } else {
                                " (no audio expected; stream present)"
                            }
                        ),
                    ));
                }
            }
        }
    }

    // Silent stretches inside the mix.
    if !expect.qa.audio_expected {
        checks.push(episode_check(
            "silence-stretches",
            EpisodeCheckStatus::Unmeasured,
            "no audio expected of this export — stretches not judged".to_string(),
        ));
    } else {
        match &m.audio_scan {
            None => checks.push(episode_check(
                "silence-stretches",
                EpisodeCheckStatus::Unmeasured,
                "audio could not be decoded".to_string(),
            )),
            Some(scan) => {
                let bad: Vec<(f32, f32)> = scan
                    .silence_stretches
                    .iter()
                    .copied()
                    .filter(|(a, b)| b - a >= params.silence_min_secs)
                    .collect();
                if bad.is_empty() {
                    let longest = scan
                        .silence_stretches
                        .iter()
                        .map(|(a, b)| b - a)
                        .fold(0.0f32, f32::max);
                    checks.push(episode_check(
                        "silence-stretches",
                        EpisodeCheckStatus::Pass,
                        format!(
                            "no silent stretch of {:.1}s or more (longest {:.1}s)",
                            params.silence_min_secs, longest
                        ),
                    ));
                } else {
                    let (a, b) = bad[0];
                    checks.push(episode_check(
                        "silence-stretches",
                        EpisodeCheckStatus::Fail,
                        format!(
                            "audio silent from {:.1}s to {:.1}s ({:.1}s, at the {:.1}s parameter){}",
                            a,
                            b,
                            b - a,
                            params.silence_min_secs,
                            if bad.len() > 1 {
                                format!("; {} such stretches", bad.len())
                            } else {
                                String::new()
                            }
                        ),
                    ));
                }
            }
        }
    }

    // Black and frozen stretches in the frame scan.
    if m.frame_samples.is_empty() {
        for name in ["black-stretches", "frozen-stretches"] {
            checks.push(episode_check(
                name,
                EpisodeCheckStatus::Unmeasured,
                "no frames could be sampled from the file".to_string(),
            ));
        }
    } else {
        let black_runs: Vec<(f32, f32)> =
            sample_runs(&m.frame_samples, m.frame_interval_secs, |s| {
                s.luma <= params.black_luma
            })
            .into_iter()
            .filter(|(a, b)| b - a >= params.black_min_secs)
            .collect();
        if black_runs.is_empty() {
            let darkest = m
                .frame_samples
                .iter()
                .map(|s| s.luma)
                .fold(1.0f32, f32::min);
            checks.push(episode_check(
                "black-stretches",
                EpisodeCheckStatus::Pass,
                format!(
                    "no black stretch of {:.1}s or more across {} samples (darkest sample luma {:.2})",
                    params.black_min_secs,
                    m.frame_samples.len(),
                    darkest
                ),
            ));
        } else {
            let (a, b) = black_runs[0];
            checks.push(episode_check(
                "black-stretches",
                EpisodeCheckStatus::Fail,
                format!(
                    "black frames from {:.1}s to {:.1}s ({:.1}s, at the {:.1}s parameter){}",
                    a,
                    b,
                    b - a,
                    params.black_min_secs,
                    if black_runs.len() > 1 {
                        format!("; {} such stretches", black_runs.len())
                    } else {
                        String::new()
                    }
                ),
            ));
        }
        if m.frame_samples.len() < 2 {
            checks.push(episode_check(
                "frozen-stretches",
                EpisodeCheckStatus::Unmeasured,
                "fewer than two frames could be sampled".to_string(),
            ));
        } else {
            let mut frozen: Vec<(f32, f32)> = Vec::new();
            let mut run_start = 0usize;
            for i in 1..=m.frame_samples.len() {
                if i == m.frame_samples.len()
                    || m.frame_samples[i].fingerprint != m.frame_samples[i - 1].fingerprint
                {
                    if i - run_start >= 2 {
                        frozen.push((
                            m.frame_samples[run_start].at_secs - m.frame_interval_secs / 2.0,
                            m.frame_samples[i - 1].at_secs + m.frame_interval_secs / 2.0,
                        ));
                    }
                    run_start = i;
                }
            }
            let bad: Vec<(f32, f32)> = frozen
                .into_iter()
                .filter(|(a, b)| b - a >= params.frozen_min_secs)
                .collect();
            if bad.is_empty() {
                checks.push(episode_check(
                    "frozen-stretches",
                    EpisodeCheckStatus::Pass,
                    format!(
                        "no frozen stretch of {:.1}s or more across {} samples",
                        params.frozen_min_secs,
                        m.frame_samples.len()
                    ),
                ));
            } else {
                let (a, b) = bad[0];
                checks.push(episode_check(
                    "frozen-stretches",
                    EpisodeCheckStatus::Fail,
                    format!(
                        "picture frozen from {:.1}s to {:.1}s ({:.1}s, at the {:.1}s parameter){}",
                        a,
                        b,
                        b - a,
                        params.frozen_min_secs,
                        if bad.len() > 1 {
                            format!("; {} such stretches", bad.len())
                        } else {
                            String::new()
                        }
                    ),
                ));
            }
        }
    }

    // Subtitle presence (per-cue burn-in spot-checks)
    // and coverage, reported as a measured fact.
    let coverage = episode_cue_coverage(expect);
    if expect.cue_facts.is_empty() {
        checks.push(episode_check(
            "subtitles",
            EpisodeCheckStatus::Pass,
            "the project carries no cues — no burn-in expected".to_string(),
        ));
    } else if expect.cue_targets.is_empty() {
        checks.push(episode_check(
            "subtitles",
            EpisodeCheckStatus::Unmeasured,
            format!(
                "every cue lies outside the timeline — no in-file spot-check possible; coverage {:.0}% of the episode",
                coverage * 100.0
            ),
        ));
    } else {
        let measured: Vec<usize> = m.cue_ink.iter().copied().flatten().collect();
        if measured.is_empty() {
            checks.push(episode_check(
                "subtitles",
                EpisodeCheckStatus::Unmeasured,
                format!(
                    "cue frames could not be decoded; coverage {:.0}% of the episode",
                    coverage * 100.0
                ),
            ));
        } else {
            let missing: Vec<usize> = expect
                .cue_targets
                .iter()
                .zip(m.cue_ink.iter())
                .filter(|(_, ink)| matches!(ink, Some(v) if *v < params.ink_min))
                .map(|(fact_idx, _)| expect.cue_facts[*fact_idx].index)
                .collect();
            if missing.is_empty() {
                checks.push(episode_check(
                    "subtitles",
                    EpisodeCheckStatus::Pass,
                    format!(
                        "burn-in present at {}/{} spot-checked cues; coverage {:.0}% of the episode",
                        measured.len(),
                        expect.cue_targets.len(),
                        coverage * 100.0
                    ),
                ));
            } else {
                checks.push(episode_check(
                    "subtitles",
                    EpisodeCheckStatus::Fail,
                    format!(
                        "subtitle burn-in missing at cue {} ({} of {} spot-checked cues missing); coverage {:.0}% of the episode",
                        missing[0] + 1,
                        missing.len(),
                        expect.cue_targets.len(),
                        coverage * 100.0
                    ),
                ));
            }
        }
    }
    checks.push(timing);

    // Title cards, where the settings demand them.
    if !expect.title_cards_enabled {
        checks.push(episode_check(
            "title-cards",
            EpisodeCheckStatus::Pass,
            "title cards are off in the project settings — none expected".to_string(),
        ));
    } else if expect.cards.is_empty() {
        checks.push(episode_check(
            "title-cards",
            if expect.chapters.is_empty() {
                EpisodeCheckStatus::Pass
            } else {
                EpisodeCheckStatus::Unmeasured
            },
            if expect.chapters.is_empty() {
                "no chapters — no cards expected".to_string()
            } else {
                "no card window falls inside the episode".to_string()
            },
        ));
    } else {
        let measured: Vec<usize> = m.card_ink.iter().copied().flatten().collect();
        if measured.is_empty() {
            checks.push(episode_check(
                "title-cards",
                EpisodeCheckStatus::Unmeasured,
                "card frames could not be decoded".to_string(),
            ));
        } else {
            let missing: Vec<&EpisodeCardExpectation> = expect
                .cards
                .iter()
                .zip(m.card_ink.iter())
                .filter(|(_, ink)| matches!(ink, Some(v) if *v < params.ink_min))
                .map(|(card, _)| card)
                .collect();
            if missing.is_empty() {
                checks.push(episode_check(
                    "title-cards",
                    EpisodeCheckStatus::Pass,
                    format!(
                        "title cards present for {}/{} chapters",
                        measured.len(),
                        expect.cards.len()
                    ),
                ));
            } else {
                let card = missing[0];
                checks.push(episode_check(
                    "title-cards",
                    EpisodeCheckStatus::Fail,
                    format!(
                        "title card missing for chapter {} (\"{}\") at {:.1}s ({} of {} chapters missing)",
                        card.chapter + 1,
                        card.title,
                        card.at_secs,
                        missing.len(),
                        expect.cards.len()
                    ),
                ));
            }
        }
    }

    // Chapter markers: the file's track against the
    // markers the export promised.
    match &m.file_chapters {
        None => checks.push(episode_check(
            "chapters",
            EpisodeCheckStatus::Unmeasured,
            "the file's chapter track could not be read".to_string(),
        )),
        Some(file_chapters) => {
            if expect.chapters.is_empty() {
                checks.push(episode_check(
                    "chapters",
                    EpisodeCheckStatus::Pass,
                    format!(
                        "no chapter markers expected; the file carries {}",
                        file_chapters.len()
                    ),
                ));
            } else if file_chapters.len() != expect.chapters.len() {
                checks.push(episode_check(
                    "chapters",
                    EpisodeCheckStatus::Fail,
                    format!(
                        "{} chapter markers in the file, {} expected",
                        file_chapters.len(),
                        expect.chapters.len()
                    ),
                ));
            } else {
                let mut problem: Option<String> = None;
                for (i, (want, got)) in expect
                    .chapters
                    .iter()
                    .zip(file_chapters.iter())
                    .enumerate()
                {
                    if got.title != want.title {
                        problem = Some(format!(
                            "chapter {} is titled \"{}\", expected \"{}\"",
                            i + 1,
                            got.title,
                            want.title
                        ));
                        break;
                    }
                    if (got.start_secs - want.start_secs).abs() > params.chapter_tol_secs {
                        problem = Some(format!(
                            "chapter {} starts at {:.1}s, expected {:.1}s",
                            i + 1,
                            got.start_secs,
                            want.start_secs
                        ));
                        break;
                    }
                }
                match problem {
                    Some(p) => checks.push(episode_check(
                        "chapters",
                        EpisodeCheckStatus::Fail,
                        p,
                    )),
                    None => checks.push(episode_check(
                        "chapters",
                        EpisodeCheckStatus::Pass,
                        format!(
                            "{} chapter markers, titles verbatim, starts within {:.1}s",
                            expect.chapters.len(),
                            params.chapter_tol_secs
                        ),
                    )),
                }
            }
        }
    }

    episode_report(dest, checks)
}

/// The fraction of the episode's timeline any cue
/// covers (ranges clamped to the timeline, overlaps
/// merged) — a reported fact, judged by nothing.
fn episode_cue_coverage(expect: &EpisodeExpectations) -> f32 {
    if expect.total_frames == 0 {
        return 0.0;
    }
    let mut spans: Vec<(usize, usize)> = expect
        .cue_facts
        .iter()
        .map(|f| (f.start_frame, f.end_frame.min(expect.total_frames)))
        .filter(|(a, b)| b > a)
        .collect();
    spans.sort_unstable();
    let mut covered = 0usize;
    let mut open: Option<(usize, usize)> = None;
    for (a, b) in spans {
        match open {
            Some((oa, ob)) if a <= ob => open = Some((oa, ob.max(b))),
            Some((oa, ob)) => {
                covered += ob - oa;
                open = Some((a, b));
            }
            None => open = Some((a, b)),
        }
    }
    if let Some((oa, ob)) = open {
        covered += ob - oa;
    }
    covered as f32 / expect.total_frames as f32
}

/// The cue-timing check: boundary sanity judged,
/// reading speed and overlaps reported as facts
/// (D-9 — no threshold verdict on either).
fn episode_cue_timing_check(
    expect: &EpisodeExpectations,
    params: &EpisodeQaParams,
) -> EpisodeCheck {
    let mut problems: Vec<String> = Vec::new();
    for fact in &expect.cue_facts {
        if fact.end_frame <= fact.start_frame {
            problems.push(format!(
                "cue {} has an empty range (frames {}–{})",
                fact.index + 1,
                fact.start_frame + 1,
                fact.end_frame
            ));
        } else if fact.start_frame >= expect.total_frames {
            problems.push(format!(
                "cue {} starts past the episode end (frame {} of {})",
                fact.index + 1,
                fact.start_frame + 1,
                expect.total_frames
            ));
        } else if fact.end_frame > expect.total_frames {
            problems.push(format!(
                "cue {} runs past the episode end (ends at frame {} of {})",
                fact.index + 1,
                fact.end_frame,
                expect.total_frames
            ));
        }
    }
    let mut facts: Vec<String> = Vec::new();
    if let Some(fastest) = expect
        .cue_facts
        .iter()
        .filter(|f| f.end_frame > f.start_frame)
        .max_by(|a, b| a.cps.partial_cmp(&b.cps).unwrap_or(std::cmp::Ordering::Equal))
    {
        facts.push(format!(
            "fastest cue {} reads {:.1} chars/s against the {:.1} reference — reported, not judged (roadmap D-9)",
            fastest.index + 1,
            fastest.cps,
            params.cps_reference
        ));
    }
    let overlaps = expect
        .cue_facts
        .iter()
        .filter(|a| {
            expect
                .cue_facts
                .iter()
                .any(|b| b.index > a.index && b.start_frame < a.end_frame && a.start_frame < b.end_frame)
        })
        .count();
    if overlaps > 0 {
        facts.push(format!("{overlaps} cue(s) overlap another cue"));
    }
    let evidence = match (problems.is_empty(), facts.is_empty()) {
        (true, true) => "every cue sits inside the timeline".to_string(),
        (true, false) => facts.join("; "),
        (false, true) => problems.join("; "),
        (false, false) => format!("{}; {}", problems.join("; "), facts.join("; ")),
    };
    episode_check(
        "cue-timing",
        if problems.is_empty() {
            EpisodeCheckStatus::Pass
        } else {
            EpisodeCheckStatus::Fail
        },
        evidence,
    )
}

/// Roll named checks into the report: failed iff any
/// check failed; the summary counts each outcome.
fn episode_report(dest: &str, checks: Vec<EpisodeCheck>) -> EpisodeQaReport {
    let count = |status: EpisodeCheckStatus| {
        checks.iter().filter(|c| c.status == status).count()
    };
    let (passed_n, failed_n, unmeasured_n) = (
        count(EpisodeCheckStatus::Pass),
        count(EpisodeCheckStatus::Fail),
        count(EpisodeCheckStatus::Unmeasured),
    );
    EpisodeQaReport {
        dest: dest.to_string(),
        passed: failed_n == 0,
        summary: format!(
            "{} checks · {} passed · {} failed · {} unmeasured",
            checks.len(),
            passed_n,
            failed_n,
            unmeasured_n
        ),
        checks,
    }
}

/// Probe one finished episode export end to end: the
/// expectations off the project, then the measurements
/// — RM29's stream probe, the audio stretch scan, the
/// frame stretch scan, the chapter track, and the cue
/// and card ink spot-checks. Gathering is best-effort
/// per measurement; the verdict decides what absence
/// means. Panel surfacing rides the W2-44 worker
/// pattern — filed as an integrator action, not built
/// here.
pub fn probe_episode(
    path: &Path,
    project: &crate::model::Project,
    params: &EpisodeQaParams,
) -> EpisodeQaReport {
    let expect = episode_expectations(project);
    let probe = probe_video_file(path);
    let mut m = EpisodeMeasurements {
        probe: probe.clone(),
        ..Default::default()
    };
    if let Some(p) = &probe {
        if p.has_audio {
            m.audio_scan = decode_audio_samples(path)
                .map(|(samples, rate)| audio_scan(&samples, rate, params.silence_peak));
        }
        if p.has_video {
            let (samples, interval) =
                episode_frame_samples(path, p.duration_secs, params.frame_interval_secs);
            m.frame_samples = samples;
            m.frame_interval_secs = interval;
            let aspect = (p.width, p.height);
            m.cue_ink = expect
                .cue_targets
                .iter()
                .map(|fact_idx| {
                    let fact = &expect.cue_facts[*fact_idx];
                    let burn = QaBurnIn {
                        at_secs: project.seconds_at_frame_time(
                            ((fact.start_frame + fact.end_frame) / 2) as f32,
                        ),
                        text_rgb: {
                            let rgb = project.subtitle_style.sanitised().text_color;
                            [
                                ((rgb >> 16) & 0xFF) as u8,
                                ((rgb >> 8) & 0xFF) as u8,
                                (rgb & 0xFF) as u8,
                            ]
                        },
                        position: project.subtitle_style.sanitised().position,
                    };
                    burn_in_ink(path, &burn, aspect)
                })
                .collect();
            m.card_ink = expect
                .cards
                .iter()
                .map(|card| card_ink_at(path, card.at_secs, card.text_rgb, aspect))
                .collect();
        }
        m.file_chapters = probe_file_chapters(path);
    }
    episode_qa_verdict(&format!("./{}", display_name(path)), &expect, &m, params)
}

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
