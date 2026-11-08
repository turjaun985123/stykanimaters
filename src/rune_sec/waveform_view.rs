//! Waveform view for the Rune editor shell (Phase D,
//! section 16 of the GPUI→Rune migration): one canvas drawing
//! the last audio clip's waveform, with a playhead line at the
//! current frame. Bin-crate code — not part of the engine
//! library. The original builders here were deliberately free
//! of `crate::` references so they could be mounted wherever
//! the shell declared them; Phase G (section 10) added the
//! first `crate::State`-taking builder
//! ([`waveform_el_state`], click-to-seek), following the same
//! pattern as every other section file, and Phase H
//! (section 27) removed the core-only builders it superseded
//! (see "One x mapping" below) — [`waveform_el_state`] is
//! now the only builder in this module.
//!
//! ## Data source (no decoding is invented here)
//!
//! The GPUI app obtained peaks exactly one way, and this module
//! keeps that path: `probe_audio_peaks` (deleted `src/app.rs`
//! at commit `1ea3633`, line 5403) read the clip's file with
//! `crate::audio_edit::read_wav` (`src/audio_edit.rs:41`) and
//! reduced it with `crate::audio_edit::peaks`
//! (`src/audio_edit.rs:537` — max-abs amplitude per bucket
//! across all channels), keeping `WavData::duration_secs`
//! (`src/audio_edit.rs:31`) alongside. The view-side peaks
//! cache the GPUI app kept lived in the deleted `EditorView`,
//! not in the core, and the first Rune version of this module
//! re-read the WAV on every build — worst during playback,
//! where the strip rebuilds per frame. Phase E restores a
//! cache (see [`clip_peaks`]): probed peaks live in a
//! thread-local map keyed by path + file length/mtime +
//! bucket count, so an unchanged file is decoded once, not
//! once per frame. Only successful probes are cached — a
//! file that appears later must show up. An unreadable file
//! yields no waveform (centre line only), the GPUI panel's
//! "no error UI" behaviour.
//!
//! Which clip: the core's audio ops are the `audio_*_last`
//! family (e.g. `src/editor.rs:2913`), so the strip shows
//! `project.audio.last()` — the same clip those ops would edit.
//!
//! ## Drawing
//!
//! Peaks are requested at 4× the display column count and
//! reduced by [`reduce_columns`] to one `(min, max)` pair per
//! column. Each column draws two mirrored bars: the outer bar
//! at `max` (accent, translucent) and the inner bar at `min`
//! (accent, opaque) — so both envelope extremes stay visible
//! after reduction. Amplitudes are scaled by the clip's `gain`
//! (`src/model.rs:505`, `AudioClip::gain`) and clamped to the
//! strip's half-height. The playhead is the project-mapped
//! marker described in the next section.
//!
//! ## One x mapping (Phase G, section 10; resolved Phase H, section 27)
//!
//! Phase G left this module with two builders using two
//! different x mappings: the original core-only `waveform_el`
//! drew a **clip-mapped** playhead (the current frame mapped
//! into the last clip's own span, via `playhead_fraction`),
//! while the interactive [`waveform_el_state`] treated the
//! strip as a miniature **project timeline**. Phase G's
//! integrator switched the shell's mount to
//! `waveform_el_state`, and a repo-wide grep at Phase H
//! (section 27) found no remaining caller of `waveform_el`
//! anywhere in `src/` — so the old builder, its
//! `waveform_shape`, and `playhead_fraction` were removed.
//! The two-mapping situation Phase G documented is resolved:
//! one builder, one mapping.
//!
//! The surviving builder ([`waveform_el_state`]) treats the
//! strip as a project timeline for interaction:
//! [`playhead_x`] maps a project frame to x with frame 0 at
//! the left edge and the last frame at the right edge, and
//! [`frame_at_x`] is its round-to-nearest inverse. The
//! builder draws its playhead with `playhead_x` and seeks
//! with `frame_at_x`, so marker and press always agree: a
//! press lands on the frame whose marker would stand there.
//! The waveform bars themselves are still the last clip's
//! peaks stretched across the whole strip width whatever the
//! clip's start frame or length, so the playhead generally
//! does NOT line up with a feature in the waveform unless
//! the clip spans the whole project — the waveform is
//! clip-time, the interaction is project-time. That
//! divergence is deliberate (it is what makes the strip
//! seekable).
//!
//! ## Selected-clip mode (Phase I, section 17)
//!
//! When the audio panel has a clip selected
//! (`State::audio_panel.selected`, Phase G section 9) and
//! that clip's own peaks exist, the strip draws the
//! SELECTED clip instead of the last one:
//! [`waveform_mode`] decides from the selection, the clip
//! count and the selected clip's peaks at the draw bucket
//! count (probed via `audio_ops::clip_peaks`, the library
//! home Phase I section 4 gave the per-clip computation;
//! gain is NOT applied there — the bars still scale by the
//! clip's gain through [`bar_span`], as always). A caption
//! element (`waveform-caption`, "Clip {n}", 1-based like
//! the panel's own numbering) names the mode under the
//! strip; project mode grows no caption, so its element
//! tree is exactly what it was before this section.
//!
//! Deliberate, do not "fix": the playhead and the seek
//! gesture keep the PROJECT mapping in both modes
//! ([`playhead_x`] / [`frame_at_x`] over the whole
//! timeline). Clip mode changes only which peaks the bars
//! draw — it is a display of the selected clip's shape,
//! not a second timeline — so the marker and the press
//! keep agreeing with the rest of the shell in both modes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

use rune::tok::type_;
use rune::{canvas, col, text, Color, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::AudioClip;

use crate::State;

/// Waveform accent (the 2.0 palette's `C_ACCENT`).
const WAVE_HEX: u32 = 0x6D8CFF;
/// Centre line / border tone (the 2.0 palette's border).
const CENTRE_HEX: u32 = 0x303744;
/// Playhead tone (the stage's selected-joint amber).
const PLAYHEAD_HEX: u32 = 0xFFC65D;
/// Caption tone (the 2.0 palette's secondary text).
const CAPTION_HEX: u32 = 0xA2AAB8;
/// Height the caption row takes under the strip in
/// selected-clip mode (Phase I, section 17): the canvas
/// gives up this much of its height so canvas + caption
/// together keep the strip's usual footprint.
const CAPTION_STRIP_H: f32 = 18.0;

/// Convert a `0xRRGGBB` colour to a Rune colour (the same
/// helper `stage_view` carries; duplicated so this section
/// file is self-contained).
fn hex_color(rgb: u32) -> Option<Color> {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF))
}

/// Reduce a peak slice to exactly `columns` `(min, max)` pairs
/// by integer partition: column `i` covers
/// `peaks[i * len / columns .. (i + 1) * len / columns]`, so
/// every peak lands in exactly one column (the partition the
/// deleted `ui::waveform::downsample_peaks` used). Deliberate
/// edge cases:
///
/// - `columns == 0` yields an empty vec.
/// - An empty `peaks` slice yields `columns` `(0.0, 0.0)`
///   pairs, so a silent/unknown clip still has the right shape.
/// - When `peaks` is shorter than `columns`, some columns cover
///   no peaks at all; they read `(0.0, 0.0)`.
/// - Non-finite peaks are read as `0.0` rather than poisoning
///   the min/max fold.
pub fn reduce_columns(peaks: &[f32], columns: usize) -> Vec<(f32, f32)> {
    if columns == 0 {
        return Vec::new();
    }
    let len = peaks.len();
    let mut out = Vec::with_capacity(columns);
    for i in 0..columns {
        let start = i * len / columns;
        let end = (i + 1) * len / columns;
        let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in &peaks[start..end] {
            let v = if v.is_finite() { v } else { 0.0 };
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
        out.push(if min <= max { (min, max) } else { (0.0, 0.0) });
    }
    out
}

/// The vertical span `(top, bottom)`, in element points, of a
/// mirrored waveform bar for amplitude `amp` scaled by the
/// clip's `gain`, centred on `mid` with at most `half` points
/// of travel. The gained amplitude is clamped to `0.0..=1.0`,
/// so an over-unity gain saturates at the strip edge instead
/// of drawing outside it.
pub fn bar_span(amp: f32, gain: f32, mid: f32, half: f32) -> (f32, f32) {
    let a = (amp * gain).clamp(0.0, 1.0) * half;
    (mid - a, mid + a)
}

/// The x position, in element points, of project frame
/// `frame` on a strip of `width` points spanning `count`
/// frames (the project mapping — see the module docs):
/// frame 0 stands at the left edge (x = 0) and the last
/// frame at the right edge (x = `width`), evenly spaced
/// between. Out-of-range frames clamp to the last frame's
/// position. Degenerate inputs — no frames, a single frame,
/// or a non-positive/non-finite width — read as 0.0: a
/// one-frame project has its only frame at the left edge,
/// and a strip with no width has nowhere else to stand.
pub fn playhead_x(frame: usize, count: usize, width: f32) -> f32 {
    if count <= 1 || width <= 0.0 || !width.is_finite() {
        return 0.0;
    }
    let f = frame.min(count - 1) as f32;
    (f / (count - 1) as f32 * width).clamp(0.0, width)
}

/// The project frame at strip position `x` — the
/// round-to-nearest inverse of [`playhead_x`]: x is clamped
/// into the strip, scaled onto the `count - 1` intervals
/// between `count` frames, and rounded, so pressing the
/// right half of the gap between two frame markers seeks
/// the later frame. Every degenerate input (no frames, a
/// single frame, non-positive width, non-finite x) reads as
/// frame 0, which is always a valid `goto` target when the
/// project has any frames at all — and `EditorCore::goto`
/// ignores the call entirely when it has none.
pub fn frame_at_x(x: f32, width: f32, frame_count: usize) -> usize {
    if frame_count <= 1 || width <= 0.0 || !width.is_finite() || !x.is_finite() {
        return 0;
    }
    let t = (x / width).clamp(0.0, 1.0);
    (t * (frame_count - 1) as f32).round() as usize
}

/// Cache key for one probe: everything that can change its
/// result. `len` + `mtime` come from one `fs::metadata` stat
/// of the file at probe time — cheap next to decoding the
/// WAV, and together they catch rewrites (an edit that keeps
/// the exact byte length still moves the mtime).
#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: String,
    len: u64,
    mtime: SystemTime,
    buckets: usize,
}

/// Cap on cached probes. The strip only ever shows one clip
/// (the last), and a clip is probed at few bucket counts, so
/// a handful of entries covers real use. When an insert
/// finds the map full it is cleared wholesale — deliberately
/// simple rather than an LRU: eviction precision buys
/// nothing at this size, and the worst case is one re-decode.
const PEAKS_CACHE_CAP: usize = 8;

thread_local! {
    /// Probed peaks + durations by [`CacheKey`], shared via
    /// `Rc` so a hit clones the tuple, not the map entry's
    /// identity. Thread-local like the clips panel's state
    /// (`clips_panel.rs`): the strip is built on the UI thread
    /// only, and per-thread storage keeps parallel tests
    /// isolated from each other.
    static PEAKS_CACHE: RefCell<HashMap<CacheKey, Rc<(Vec<f32>, f32)>>> =
        RefCell::new(HashMap::new());
}

/// The key for `path` at `buckets`, from a fresh stat.
/// `None` when the file cannot be statted or its mtime read
/// (usually: it does not exist).
fn cache_key(path: &str, buckets: usize) -> Option<CacheKey> {
    let md = std::fs::metadata(path).ok()?;
    Some(CacheKey {
        path: path.to_string(),
        len: md.len(),
        mtime: md.modified().ok()?,
        buckets,
    })
}

fn cache_get(key: &CacheKey) -> Option<(Vec<f32>, f32)> {
    PEAKS_CACHE.with(|c| c.borrow().get(key).map(|hit| (**hit).clone()))
}

/// Last-resort lookup for a file that cannot be statted: any
/// cached probe of the same path at the same bucket count,
/// whatever its recorded len/mtime. If several generations of
/// the file are cached, whichever the map yields first wins —
/// the peaks of a vanished file are a display nicety, not
/// data worth versioning.
fn cache_get_stale(clip: &AudioClip, buckets: usize) -> Option<(Vec<f32>, f32)> {
    PEAKS_CACHE.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| k.path == clip.path && k.buckets == buckets)
            .map(|(_, hit)| (**hit).clone())
    })
}

fn cache_insert(key: CacheKey, probed: (Vec<f32>, f32)) {
    PEAKS_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() >= PEAKS_CACHE_CAP {
            cache.clear();
        }
        cache.insert(key, Rc::new(probed));
    });
}

/// Probe one clip's peaks the way the GPUI app did: read the
/// WAV via `audio_edit::read_wav`, reduce with
/// `audio_edit::peaks` at `buckets` buckets, and return the
/// peaks plus the clip's duration in seconds (the duration
/// caches alongside the peaks, in the same entry). `None`
/// when the file cannot be read (missing, not PCM16 WAV, …)
/// — callers draw no waveform for that clip.
///
/// Successful probes are cached in [`PEAKS_CACHE`] under the
/// file's current (path, len, mtime, buckets); a later call
/// with an unchanged file is served from the map and never
/// touches the decoder. Failures are never cached, so a file
/// that appears (or becomes readable) later probes fresh.
/// One race is accepted: if the file changes between the
/// stat and the read, the peaks are stored under the
/// pre-read key — the next call stats the new len/mtime,
/// misses, and re-probes, so the entry self-corrects.
///
/// When the file cannot be statted at all (deleted, usually),
/// a previous successful probe of the same path + buckets is
/// served stale via [`cache_get_stale`] instead of blanking
/// the strip mid-session; a file that reappears stats into a
/// fresh key and re-probes then.
pub fn clip_peaks(clip: &AudioClip, buckets: usize) -> Option<(Vec<f32>, f32)> {
    let key = cache_key(&clip.path, buckets);
    match &key {
        Some(key) => {
            if let Some(hit) = cache_get(key) {
                return Some(hit);
            }
        }
        None => {
            if let Some(hit) = cache_get_stale(clip, buckets) {
                return Some(hit);
            }
        }
    }
    let data = stykanimate_rs::audio_edit::read_wav(&clip.path).ok()?;
    let duration_secs = data.duration_secs();
    let probed = (stykanimate_rs::audio_edit::peaks(&data, buckets), duration_secs);
    if let Some(key) = key {
        cache_insert(key, probed.clone());
    }
    Some(probed)
}

/// Which clip's peaks the strip draws (Phase I,
/// section 17): the project view (the last clip, as
/// always) or one selected clip's own peaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveformMode {
    /// The last clip's peaks stretched across the strip —
    /// the strip's behaviour since Phase D.
    Project,
    /// The selected clip's own peaks (0-based clip index,
    /// a valid index into `project.audio`).
    Clip(usize),
}

/// The strip's mode decision, over plain values so it is
/// testable without a `State` (this file's pure-function
/// style — cf. [`playhead_x`]): [`WaveformMode::Clip`]
/// when `selected` is `Some(i)`, `i` names an existing
/// clip (`i < clip_count`), and that clip's peaks at the
/// draw bucket count are non-empty; [`WaveformMode::Project`]
/// otherwise. A stale selection (the project shrank under
/// it) and a selected clip whose file will not decode
/// (empty peaks) both read as Project — the strip falls
/// back to the last clip rather than blanking. Note the
/// peaks slice decides on LENGTH only: a readable but
/// silent clip probes as all-zero peaks, which is a
/// non-empty slice and quite rightly Clip mode.
pub fn waveform_mode(
    selected: Option<usize>,
    clip_count: usize,
    selected_peaks: &[f32],
) -> WaveformMode {
    match selected {
        Some(i) if i < clip_count && !selected_peaks.is_empty() => WaveformMode::Clip(i),
        _ => WaveformMode::Project,
    }
}

/// The caption naming the strip's mode, or `None` when no
/// caption element is built at all: `"Clip {n}"` in clip
/// mode, with `n` 1-based (the audio panel's own numbering
/// — its op statuses count clips from 1), and `None` in
/// project mode, whose element tree stays exactly what it
/// was before Phase I (section 17).
pub fn waveform_caption(mode: WaveformMode) -> Option<String> {
    match mode {
        WaveformMode::Clip(i) => Some(format!("Clip {}", i + 1)),
        WaveformMode::Project => None,
    }
}

/// Display columns for a strip `w` points wide: one column
/// per ~3 points (2 pt bar + 1 pt gap), capped so a very
/// wide strip stays cheap. Factored out of
/// [`waveform_bars`] so the builder probes selected-clip
/// peaks at exactly the bucket count the bars will draw.
fn strip_columns(w: f32) -> usize {
    ((w / 3.0).floor() as usize).clamp(1, 512)
}

/// Probe buckets for `columns` display columns: peaks are
/// probed at 4× the column count so [`reduce_columns`]
/// still sees local extremes.
fn strip_buckets(columns: usize) -> usize {
    (columns * 4).clamp(96, 2048)
}

/// The waveform strip's bars as one `Shape` in
/// canvas-element points, for the last audio clip in the
/// project: centre line plus one column of mirrored bars per
/// ~3 points of width. Bars are drawn with [`LineCap::Butt`]
/// so a bar's ends land exactly on its amplitude (round caps
/// would overshoot by half the bar width). With no clips —
/// or an unreadable clip file — the shape is just the centre
/// line, so the strip reads as an empty audio lane rather
/// than a hole in the layout. No playhead here:
/// [`waveform_shape_project`] appends the one
/// project-mapped marker.
fn waveform_bars(core: &EditorCore, el_size: [f32; 2]) -> Shape {
    let (w, h) = (el_size[0], el_size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let mid = h / 2.0;
    let centre = hex_color(CENTRE_HEX).unwrap_or(Color::WHITE);
    shape = shape.line([0.0, mid], [w, mid], 1.0, LineCap::Butt, centre);

    let Some(clip) = core.project.audio.last() else {
        return shape;
    };
    let buckets = strip_buckets(strip_columns(w));
    let Some((peaks, _)) = clip_peaks(clip, buckets) else {
        return shape;
    };
    draw_peak_bars(shape, &peaks, clip.gain, w, h)
}

/// Append the mirrored bar columns for `peaks` (reduced to
/// the strip's display columns, amplitudes scaled by
/// `gain` through [`bar_span`]) to `shape` — the drawing
/// tail [`waveform_bars`] and [`waveform_bars_clip`] share,
/// so project mode and selected-clip mode render peaks
/// identically; only the peaks' source differs.
fn draw_peak_bars(shape: Shape, peaks: &[f32], gain: f32, w: f32, h: f32) -> Shape {
    let mut shape = shape;
    let mid = h / 2.0;
    let columns = strip_columns(w);
    let cols = reduce_columns(peaks, columns);
    let accent = hex_color(WAVE_HEX).unwrap_or(Color::WHITE);
    let outer = accent.alpha(0.5);
    let pitch = w / columns as f32;
    let bar_w = (pitch - 1.0).max(1.0);
    for (i, &(min, max)) in cols.iter().enumerate() {
        let x = (i as f32 + 0.5) * pitch;
        let (top, bottom) = bar_span(max, gain, mid, mid);
        shape = shape.line([x, top], [x, bottom], bar_w, LineCap::Butt, outer);
        let (top, bottom) = bar_span(min, gain, mid, mid);
        shape = shape.line([x, top], [x, bottom], bar_w, LineCap::Butt, accent);
    }
    shape
}

/// The selected-clip counterpart of [`waveform_bars`]
/// (Phase I, section 17): centre line plus bar columns for
/// clip `index`'s own `peaks` (probed by the builder via
/// `audio_ops::clip_peaks`, gain NOT applied there), scaled
/// by that clip's `gain`. The mode decision has already
/// proven the index valid and the peaks non-empty; the
/// index is still re-checked here so this builder is safe
/// to call on its own — an out-of-range index draws the
/// centre line only, like an unreadable clip in project
/// mode.
fn waveform_bars_clip(
    core: &EditorCore,
    index: usize,
    peaks: &[f32],
    el_size: [f32; 2],
) -> Shape {
    let (w, h) = (el_size[0], el_size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let mid = h / 2.0;
    let centre = hex_color(CENTRE_HEX).unwrap_or(Color::WHITE);
    shape = shape.line([0.0, mid], [w, mid], 1.0, LineCap::Butt, centre);
    let Some(clip) = core.project.audio.get(index) else {
        return shape;
    };
    draw_peak_bars(shape, peaks, clip.gain, w, h)
}

/// The strip for [`waveform_el_state`]: the centre line and
/// bars of [`waveform_bars`], plus the project-mapped
/// playhead ([`with_playhead`]).
fn waveform_shape_project(core: &EditorCore, el_size: [f32; 2]) -> Shape {
    with_playhead(waveform_bars(core, el_size), core, el_size)
}

/// Append the project-mapped playhead to a bars shape:
/// [`playhead_x`] of the current frame, drawn whenever the
/// project has frames — including over the empty lane of a
/// project with no (readable) clip, since the strip still
/// seeks there. Drawn last, over the bars, in the stage's
/// selected-joint amber. Shared by the project shape and
/// the selected-clip shape (Phase I, section 17): the
/// playhead is project-mapped in both modes, deliberately
/// (see the module docs).
fn with_playhead(shape: Shape, core: &EditorCore, el_size: [f32; 2]) -> Shape {
    let mut shape = shape;
    let (w, h) = (el_size[0], el_size[1]);
    if w > 0.0 && h > 0.0 && core.frame_count() > 0 {
        let x = playhead_x(core.current, core.frame_count(), w);
        let playhead = hex_color(PLAYHEAD_HEX).unwrap_or(Color::WHITE);
        shape = shape.line([x, 0.0], [x, h], 2.0, LineCap::Butt, playhead);
    }
    shape
}

/// The selected-clip strip (Phase I, section 17):
/// [`waveform_bars_clip`] plus the same project-mapped
/// playhead as the project strip ([`with_playhead`]).
fn waveform_shape_clip(
    core: &EditorCore,
    index: usize,
    peaks: &[f32],
    el_size: [f32; 2],
) -> Shape {
    with_playhead(
        waveform_bars_clip(core, index, peaks, el_size),
        core,
        el_size,
    )
}

/// The seek gesture for [`waveform_el_state`]: pressing —
/// and, holding, dragging — on the strip goes to the frame
/// under the pointer (click-to-seek plus scrub, one gesture).
/// Navigation only: `EditorCore::goto` moves the playhead
/// without a snapshot and posts no status, so a scrub leaves
/// the undo stack and the status line exactly as they were
/// (the timeline chips' click semantics). The frame comes
/// from [`frame_at_x`] over the element's own laid-out width
/// (`Drag::w`, the same local geometry the drag reports), so
/// the seek follows the strip as actually drawn; a gesture
/// on a zero-width element is ignored. `Phase::Up` does
/// nothing — the last Down/Move position stands — and a
/// cancelled gesture has nothing to restore, for the same
/// reason: goto never mutated anything but the playhead.
fn waveform_seek(s: &mut State, d: &Drag) {
    match d.phase {
        Phase::Down | Phase::Move => {
            if d.w > 0.0 {
                let frame = frame_at_x(d.x, d.w, s.core.frame_count());
                s.core.goto(frame);
            }
        }
        Phase::Up => {}
    }
}

/// The interactive waveform strip — the only builder in
/// this module since Phase H (section 27) removed the
/// core-only `waveform_el` (see the module docs). The canvas
/// carries the `waveform` key (so headless `--tree` checks
/// and the shell's layout reads can find it), is drawn by
/// [`waveform_shape_project`] (project-mapped playhead) and
/// carries the seek gesture ([`waveform_seek`]).
/// Element-local drag points are in the element's drawn
/// points (Rune's `Drag` contract), matching the points the
/// shape was built in as long as the caller lays the canvas
/// out at the `el_size` it passes — the current mount passes
/// `[308.0, 56.0]` and the right column gives the canvas
/// that width.
///
/// Phase I (section 17) adds the selected-clip mode: when
/// [`waveform_mode`] reads Clip, the canvas draws the
/// selected clip's own peaks (in a canvas shortened by
/// [`CAPTION_STRIP_H`], so strip + caption keep the usual
/// footprint) and a `waveform-caption` element under it
/// names the clip. In Project mode the return is the bare
/// canvas above — the element tree is byte-identical to
/// what it was before this section. The playhead and the
/// seek gesture are project-mapped in BOTH modes,
/// deliberately (see the module docs): clip mode changes
/// only which peaks the bars draw.
pub fn waveform_el_state(s: &State, el_size: [f32; 2]) -> El {
    let clip_count = s.core.project.audio.len();
    let selected_peaks: Vec<f32> = match s.audio_panel.selected {
        Some(i) if i < clip_count => stykanimate_rs::audio_ops::clip_peaks(
            &s.core,
            i,
            strip_buckets(strip_columns(el_size[0])),
        ),
        _ => Vec::new(),
    };
    let mode = waveform_mode(s.audio_panel.selected, clip_count, &selected_peaks);
    if let (WaveformMode::Clip(i), Some(caption)) = (mode, waveform_caption(mode)) {
        let canvas_h = (el_size[1] - CAPTION_STRIP_H).max(0.0);
        return col()
            .gap(2.0)
            .child(
                canvas(waveform_shape_clip(
                    &s.core,
                    i,
                    &selected_peaks,
                    [el_size[0], canvas_h],
                ))
                .key("waveform")
                .h(canvas_h)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| waveform_seek(s, d)),
            )
            .child(
                text(caption)
                    .key("waveform-caption")
                    .font(type_::LABEL)
                    .color(hex_color(CAPTION_HEX).unwrap_or(Color::WHITE)),
            );
    }
    canvas(waveform_shape_project(&s.core, el_size))
        .key("waveform")
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| waveform_seek(s, d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::audio_edit::{peaks, write_wav, WavData};

    #[test]
    fn reduce_columns_hand_computed() {
        // Integer partition of 4 peaks into 2 columns:
        // [0.0, 1.0] and [0.5, 0.25].
        let cols = reduce_columns(&[0.0, 1.0, 0.5, 0.25], 2);
        assert_eq!(cols, vec![(0.0, 1.0), (0.25, 0.5)]);
        // Identity when columns == len.
        let cols = reduce_columns(&[0.3, 0.9], 2);
        assert_eq!(cols, vec![(0.3, 0.3), (0.9, 0.9)]);
    }

    #[test]
    fn reduce_columns_flat_input_is_flat() {
        let cols = reduce_columns(&[0.2; 7], 3);
        assert_eq!(cols, vec![(0.2, 0.2); 3]);
    }

    #[test]
    fn reduce_columns_edge_cases() {
        assert!(reduce_columns(&[1.0, 2.0], 0).is_empty());
        // Empty peaks: right shape, all silence.
        assert_eq!(reduce_columns(&[], 4), vec![(0.0, 0.0); 4]);
        // Fewer peaks than columns: uncovered columns are 0.
        let cols = reduce_columns(&[0.6], 3);
        assert_eq!(cols.len(), 3);
        assert!(cols.iter().all(|&c| c == (0.0, 0.0) || c == (0.6, 0.6)));
        assert!(cols.iter().any(|&c| c == (0.6, 0.6)));
        // Non-finite peaks read as 0.0, not NaN poisoning.
        let cols = reduce_columns(&[f32::NAN, 0.5], 1);
        assert_eq!(cols, vec![(0.0, 0.5)]);
    }

    #[test]
    fn reduce_columns_of_sine_peaks() {
        // One second of 440 Hz sine at amplitude 0.8, mono.
        let samples: Vec<f32> = (0..8000)
            .map(|i| {
                let t = i as f32 / 8000.0;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.8
            })
            .collect();
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let p = peaks(&wav, 10);
        assert_eq!(p.len(), 10);
        // Every bucket spans many cycles, so each peak sits
        // just under the 0.8 crest (discrete sampling).
        for v in &p {
            assert!(*v > 0.75 && *v <= 0.8001, "sine peak out of range: {v}");
        }
        let cols = reduce_columns(&p, 5);
        assert_eq!(cols.len(), 5);
        for (min, max) in cols {
            assert!(min > 0.75 && max <= 0.8001 && max - min < 0.05);
        }
    }

    #[test]
    fn reduce_columns_of_flat_peaks() {
        // Constant 0.3 DC: every bucket peaks at exactly 0.3.
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.3; 4000],
        };
        let p = peaks(&wav, 8);
        assert!(p.iter().all(|&v| (v - 0.3).abs() < 1e-6));
        let cols = reduce_columns(&p, 4);
        assert!(cols.iter().all(|&c| c == (0.3, 0.3)));
    }

    #[test]
    fn bar_span_scales_and_clamps_gain() {
        // Unity gain: amplitude maps linearly onto the half.
        assert_eq!(bar_span(0.5, 1.0, 24.0, 24.0), (12.0, 36.0));
        // Gain 2 doubles until it saturates at the strip edge.
        assert_eq!(bar_span(0.25, 2.0, 24.0, 24.0), (12.0, 36.0));
        assert_eq!(bar_span(0.75, 2.0, 24.0, 24.0), (0.0, 48.0));
        // Zero gain / zero amplitude collapse to the centre.
        assert_eq!(bar_span(0.9, 0.0, 24.0, 24.0), (24.0, 24.0));
        assert_eq!(bar_span(0.0, 3.0, 24.0, 24.0), (24.0, 24.0));
    }

    // ---- Peaks cache (Phase E, section 27) ----

    /// One second of 440 Hz sine at amplitude 0.8, mono 8 kHz
    /// (the same signal the reduction tests use).
    fn sine_wav() -> WavData {
        let samples: Vec<f32> = (0..8000)
            .map(|i| {
                let t = i as f32 / 8000.0;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.8
            })
            .collect();
        WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        }
    }

    fn temp_wav_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "stykanimate-rune-waveform-{}-{}.wav",
            std::process::id(),
            tag
        ))
    }

    fn clip_at(path: &std::path::Path) -> AudioClip {
        AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
        }
    }

    #[test]
    fn clip_peaks_hit_survives_file_deletion() {
        let path = temp_wav_path("cache-hit");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let c = clip_at(&path);
        let first = clip_peaks(&c, 64).expect("fresh probe");
        assert_eq!(first.0.len(), 64);
        assert_eq!(first.1, 1.0);
        assert!(first.0.iter().all(|&v| v > 0.7 && v <= 0.81));
        // Delete the file: a fresh read would now fail, so an
        // identical second result can only come from the cache.
        std::fs::remove_file(&path).unwrap();
        let second = clip_peaks(&c, 64).expect("cached probe");
        assert_eq!(first, second);
    }

    #[test]
    fn clip_peaks_miss_after_the_file_is_rewritten() {
        let path = temp_wav_path("cache-miss");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let c = clip_at(&path);
        let first = clip_peaks(&c, 64).expect("fresh probe");
        assert!(first.0.iter().copied().fold(0.0f32, f32::max) > 0.7);
        // Rewrite the same path with flat DC: same length, new
        // mtime. (The pause keeps the two writes on distinct
        // mtime ticks even on coarse-grained filesystems.)
        std::thread::sleep(std::time::Duration::from_millis(20));
        let dc = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.3; 8000],
        };
        std::fs::write(&path, write_wav(&dc)).unwrap();
        let second = clip_peaks(&c, 64).expect("re-probe after rewrite");
        assert_eq!(second.1, 1.0);
        let max = second.0.iter().copied().fold(0.0f32, f32::max);
        assert!(max < 0.35, "expected DC peaks after rewrite, got max {max}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn clip_peaks_failures_are_not_cached() {
        let path = temp_wav_path("cache-none");
        let _ = std::fs::remove_file(&path);
        let c = clip_at(&path);
        assert!(clip_peaks(&c, 64).is_none());
        // The file appears later: had the failure been cached,
        // this would still read as missing.
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let probed = clip_peaks(&c, 64).expect("probe once the file exists");
        assert_eq!(probed.0.len(), 64);
        let _ = std::fs::remove_file(&path);
    }

    // ---- Project mapping: playhead_x / frame_at_x (Phase G) ----

    #[test]
    fn playhead_x_first_middle_last() {
        // 11 frames over 300 pt: one frame every 30 pt, first
        // frame on the left edge, last on the right edge.
        assert_eq!(playhead_x(0, 11, 300.0), 0.0);
        assert_eq!(playhead_x(5, 11, 300.0), 150.0);
        assert_eq!(playhead_x(10, 11, 300.0), 300.0);
        // A second geometry, hand-computed the same way:
        // 5 frames over 100 pt, frame 2 is the middle.
        assert_eq!(playhead_x(0, 5, 100.0), 0.0);
        assert_eq!(playhead_x(2, 5, 100.0), 50.0);
        assert_eq!(playhead_x(4, 5, 100.0), 100.0);
    }

    #[test]
    fn playhead_x_degenerate_and_clamped() {
        // Past-the-end frames clamp to the last frame's spot.
        assert_eq!(playhead_x(99, 11, 300.0), 300.0);
        // No frames / one frame / no width: the left edge.
        assert_eq!(playhead_x(0, 0, 300.0), 0.0);
        assert_eq!(playhead_x(0, 1, 300.0), 0.0);
        assert_eq!(playhead_x(5, 11, 0.0), 0.0);
        assert_eq!(playhead_x(5, 11, -30.0), 0.0);
    }

    #[test]
    fn frame_at_x_rounds_to_nearest() {
        // 11 frames over 300 pt: markers every 30 pt, and the
        // switch point between two frames is the gap's middle.
        assert_eq!(frame_at_x(0.0, 300.0, 11), 0);
        assert_eq!(frame_at_x(14.0, 300.0, 11), 0);
        assert_eq!(frame_at_x(16.0, 300.0, 11), 1);
        assert_eq!(frame_at_x(44.0, 300.0, 11), 1);
        assert_eq!(frame_at_x(45.0, 300.0, 11), 2);
        assert_eq!(frame_at_x(150.0, 300.0, 11), 5);
        assert_eq!(frame_at_x(284.0, 300.0, 11), 9);
        assert_eq!(frame_at_x(286.0, 300.0, 11), 10);
        assert_eq!(frame_at_x(300.0, 300.0, 11), 10);
        // 5 frames over 100 pt: markers every 25 pt.
        assert_eq!(frame_at_x(50.0, 100.0, 5), 2);
        assert_eq!(frame_at_x(37.0, 100.0, 5), 1);
        assert_eq!(frame_at_x(38.0, 100.0, 5), 2);
    }

    #[test]
    fn frame_at_x_edges() {
        // Outside the strip clamps to the end frames.
        assert_eq!(frame_at_x(-20.0, 300.0, 11), 0);
        assert_eq!(frame_at_x(999.0, 300.0, 11), 10);
        // Zero / negative width and degenerate frame counts
        // read as frame 0, never out of range.
        assert_eq!(frame_at_x(150.0, 0.0, 11), 0);
        assert_eq!(frame_at_x(150.0, -5.0, 11), 0);
        assert_eq!(frame_at_x(150.0, 300.0, 0), 0);
        assert_eq!(frame_at_x(0.0, 300.0, 1), 0);
        assert_eq!(frame_at_x(300.0, 300.0, 1), 0);
        // Non-finite positions read as frame 0.
        assert_eq!(frame_at_x(f32::NAN, 300.0, 11), 0);
        assert_eq!(frame_at_x(f32::INFINITY, 300.0, 11), 0);
    }

    #[test]
    fn frame_at_x_inverts_playhead_x() {
        // Pressing exactly on a frame's marker seeks that
        // frame — the property that makes the new builder's
        // playhead and its seek agree.
        for f in 0..=10 {
            let x = playhead_x(f, 11, 300.0);
            assert_eq!(frame_at_x(x, 300.0, 11), f, "frame {f} at x {x}");
        }
        for f in 0..=4 {
            let x = playhead_x(f, 5, 100.0);
            assert_eq!(frame_at_x(x, 100.0, 5), f, "frame {f} at x {x}");
        }
    }

    // ---- Selected-clip mode (Phase I, section 17) ----

    #[test]
    fn waveform_mode_selection_table() {
        let peaks: &[f32] = &[0.25, 0.5];
        // No selection: project mode, whatever else is true.
        assert_eq!(waveform_mode(None, 3, peaks), WaveformMode::Project);
        assert_eq!(waveform_mode(None, 0, &[]), WaveformMode::Project);
        // Stale selection (the project shrank under it):
        // project mode even with peaks in hand.
        assert_eq!(waveform_mode(Some(3), 3, peaks), WaveformMode::Project);
        assert_eq!(waveform_mode(Some(9), 3, peaks), WaveformMode::Project);
        assert_eq!(waveform_mode(Some(0), 0, peaks), WaveformMode::Project);
        // Valid selection whose clip has no peaks (its file
        // would not decode): project mode.
        assert_eq!(waveform_mode(Some(1), 3, &[]), WaveformMode::Project);
        // Valid selection with peaks: clip mode, index kept.
        assert_eq!(waveform_mode(Some(1), 3, peaks), WaveformMode::Clip(1));
        assert_eq!(waveform_mode(Some(0), 1, peaks), WaveformMode::Clip(0));
        // A silent-but-readable clip probes as all-zero
        // peaks: a non-empty slice, so still clip mode —
        // the decision reads length, never amplitude.
        assert_eq!(
            waveform_mode(Some(2), 3, &[0.0, 0.0]),
            WaveformMode::Clip(2)
        );
    }

    #[test]
    fn waveform_caption_only_in_clip_mode() {
        // 1-based, the audio panel's own numbering.
        assert_eq!(
            waveform_caption(WaveformMode::Clip(0)).as_deref(),
            Some("Clip 1")
        );
        assert_eq!(
            waveform_caption(WaveformMode::Clip(4)).as_deref(),
            Some("Clip 5")
        );
        // Project mode grows no caption element at all.
        assert_eq!(waveform_caption(WaveformMode::Project), None);
    }

    #[test]
    fn strip_layout_columns_and_buckets() {
        // The mount's 308 pt strip: 102 columns, probed at
        // 4x (the counts the bars have always drawn with).
        assert_eq!(strip_columns(308.0), 102);
        assert_eq!(strip_buckets(102), 408);
        // The clamps at both ends.
        assert_eq!(strip_columns(0.0), 1);
        assert_eq!(strip_buckets(1), 96);
        assert_eq!(strip_columns(10_000.0), 512);
        assert_eq!(strip_buckets(512), 2048);
    }
}
