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
//! count (probed via [`selected_clip_peaks`] — Phase I
//! probed through the library's `audio_ops::clip_peaks`;
//! Wave 10 rerouted the probe through this module's
//! cache, see the Wave 10 section below; gain is NOT
//! applied there — the bars still scale by the
//! clip's gain through [`bar_span`], as always). A caption
//! element (`waveform-caption`, "Clip {n}", 1-based like
//! the panel's own numbering) names the mode under the
//! strip; project mode grows no caption, so its element
//! tree is exactly what it was before this section.
//!
//! The playhead DRAWING keeps the PROJECT mapping in
//! both modes ([`playhead_x`] over the whole timeline):
//! the marker must agree with the timeline chips and the
//! stage, which know nothing of clip spans. Phase I
//! (section 17) also kept the seek gesture project-mapped
//! in both modes, deliberately at the time — clip mode
//! was a display of the selected clip's shape, not a
//! second timeline. Phase J (section 31) revisits
//! exactly half of that: in clip mode the DRAG now seeks
//! within the selected clip's own span
//! ([`clip_seek_frame`]), because the bars under the
//! pointer are that clip's waveform — scrubbing the
//! whole timeline dragged the playhead off the feature
//! under the pointer. The marker still renders
//! project-mapped: the frame a clip-mode drag lands on
//! is an ordinary project frame inside the clip's span,
//! drawn the way every other surface draws it. When the
//! clip's length is unknown (unreadable or zero-length
//! file), the drag falls back to the project mapping
//! unchanged.
//!
//! ## Span bracket (Phase K, section 21)
//!
//! When a clip is selected and its span is known
//! ([`clip_span`] — the same cached probe the seek
//! resolves through), both shapes also draw the span as
//! a bracket over the strip: a rule at the top
//! ([`SPAN_BRACKET_Y`]) across the span's x edges
//! ([`span_bracket`]) with a short tick at each edge, in
//! the selection accent at reduced alpha, under the
//! playhead. The bracket is drawn in project coordinates
//! — where the clip sits in the project — even in clip
//! mode, because the playhead it keeps company with is
//! also project-mapped; that is the deliberate asymmetry
//! with the clip-mode seek above, which IS clip-mapped.
//! In clip mode the bracket draws anyway, over the
//! selected clip's own strip (it frames the mode); with
//! no selection, a stale selection, or an unknown span
//! it does not, and the shapes are exactly what they
//! were before this section.
//!
//! ## Clip boundary ticks (Phase L, section 36)
//!
//! Project mode additionally marks EVERY clip's
//! boundaries: for each clip whose span resolves
//! ([`clip_span`], the same cached probe, one index at
//! a time), a short tick at each span edge
//! ([`all_clip_edges`] collects the edges,
//! project-mapped through [`span_bracket`] exactly as
//! the bracket's are), drawn in the same accent at a
//! fainter alpha ([`ALL_TICKS_ALPHA`]) and WITHOUT the
//! bracket's top rule — so the selected clip's bracket
//! still reads as the strong mark and the rest read as
//! a quiet boundary rhythm under it. The draw order in
//! the project shape is bars, then all the ticks, then
//! the selected bracket (which draws over its own
//! clip's ticks), then the playhead. Clip mode is
//! untouched: its strip shows one clip, whose span the
//! bracket already frames, so a boundary rhythm would
//! only repeat it. A clip whose file will not decode
//! contributes no ticks — its span is unknown, and an
//! invented edge would be a false claim about where
//! the clip sits.
//!
//! ## Muted clips (Phase M, section 16)
//!
//! `AudioClip::muted` (Phase M) silences a clip in
//! playback and export mixes; this strip makes the
//! state VISIBLE. In clip mode a muted clip's bars keep
//! their geometry and hue but draw at
//! [`MUTED_BAR_ALPHA`] of their usual alpha
//! ([`bar_colors`]), and the caption gains its suffix
//! through [`clip_caption`]: "Clip {n} · muted" (the
//! unmuted caption is byte-identical to Phase I's).
//! In project mode the bars are untouched — they show
//! the last clip, selected or not — but the SELECTED
//! clip's span bracket dims by the same factor
//! ([`bracket_color`]) when that clip is muted; the
//! all-clip boundary ticks stay as Phase L left them,
//! muted or not. [`clip_is_muted`] is the one read of
//! the flag both paths share.
//!
//! ## Clip health caption (UI mission Wave 6-D)
//!
//! Wave 4 gave the audio panel's clip rows health
//! badges (audit D2): a clip whose file is missing, not
//! a WAV, or unreadable no longer renders identically
//! to a healthy silent clip THERE. The strip kept the
//! old behaviour one level down: a selected clip in any
//! of those states cannot decode, so its peaks come
//! back empty, [`waveform_mode`] falls back to Project,
//! and the strip showed the last clip (or a bare centre
//! line) with nothing saying the SELECTED clip is the
//! broken one. [`waveform_el_state`] now closes that:
//! in project mode with a valid selection, the selected
//! clip is run through the panel's own probe
//! ([`crate::rune_sec::audio_panel::probe_clip`], served
//! from the panel state's shared cache, so the decoder
//! still runs at most once per file change) and a
//! non-healthy result puts a caption under the strip —
//! [`clip_health_caption`]: "Clip {n} · missing file" /
//! "unsupported format" / "unreadable file", the panel's
//! [`crate::rune_sec::audio_panel::health_word`]
//! vocabulary — in the warning ink the panel's badge
//! uses ([`crate::rune_sec::app_theme::warning_on`]).
//! The caption takes the clip-mode caption's slot (same
//! `waveform-caption` key, same reserved
//! [`CAPTION_STRIP_H`]), so the strip's footprint is
//! unchanged; a healthy selected clip, a stale
//! selection and no selection all build exactly the
//! tree they built before — no caption, no geometry
//! change.
//!
//! ## Theme wiring (UI mission, Wave 2 — s02 §3.1, s14 D7)
//!
//! Until this section the strip was the shell's one
//! theme-blind surface: four file-local hex constants
//! (the frozen GPUI-era palette) coloured the bars, the
//! centre line, the playhead and the caption, and
//! [`waveform_el_state`] did not take a theme at all —
//! in the light theme the caption drew dark-theme grey
//! on a white card, the weakest text contrast in the
//! shell. The builder now takes `&Theme` (the shell's
//! mount passes the same theme every panel receives)
//! and every colour derives from the token architecture:
//! the bars, bracket and boundary ticks take the theme
//! accent ([`crate::rune_sec::app_theme::accent`]),
//! the centre line takes the theme's border token
//! (`Theme::card_border`), the playhead takes the
//! warning amber ([`playhead_color`]) and the
//! caption takes `Theme::text_secondary`. No geometry
//! moved: same bars, same alphas, same line widths —
//! only the colours' provenance changed.
//!
//! ## Wave 10 audit: long projects, probe cost, seek precision
//!
//! Three questions the earlier phases never asked of
//! this strip, answered from the source:
//!
//! - LONG PROJECTS. There is no window and nothing to
//!   follow: [`playhead_x`] maps the WHOLE project onto
//!   the strip (frame 0 at the left edge, the last frame
//!   at the right), so the playhead cannot run off the
//!   drawn region — the frame clamps to the project's
//!   last and the x to the strip, and [`with_playhead`]
//!   draws the marker whenever the project has frames.
//!   Visibility is by construction; what a long project
//!   trades away is resolution, not the marker.
//! - SEEK PRECISION. The strip is 308 pt wide at its
//!   one mount, so one frame owns 308 / (count − 1) pt
//!   of drag: 3.46 pt at 90 frames, 1.72 at 180, ~1.0
//!   at 300. [`frame_at_x`] rounds to nearest, so at
//!   the 90-frame scale every frame's target zone is
//!   several points wide and a drag is frame-exact; the
//!   pitch only reaches pointer granularity (~1 pt)
//!   around 300 frames. That is the whole-project
//!   miniature working as designed (Phases G/H), not a
//!   defect, and this audit builds no window against
//!   it — a window would also need somewhere to live,
//!   and every surface in this file (playhead, bracket,
//!   boundary ticks) is pinned to the whole-project
//!   mapping by the phases above.
//! - PROBE COST. The two modes probed differently, and
//!   one of the paths was uncached: project mode reads
//!   through this module's cached [`clip_peaks`]
//!   (Phase E), but the builder's selected-clip probe
//!   called the library's `audio_ops::clip_peaks`,
//!   which decodes the clip's whole WAV on EVERY call —
//!   and the builder calls it on every build whenever a
//!   clip is selected, which during playback is every
//!   frame. [`selected_clip_peaks`] now answers the
//!   same question through the cached probe: an
//!   unchanged file decodes once, and a file deleted
//!   mid-session serves stale, exactly the project
//!   path's Phase E behaviour. The library probe's one
//!   semantic the cached probe does not carry by
//!   itself — a file that decodes to no sample frames
//!   counts as "no peaks" there, while
//!   `audio_edit::peaks` gives it all-zero buckets — is
//!   preserved through the cached probe's duration
//!   half: zero content has a duration of exactly 0.0
//!   (the same reading `audio_ops::clip_duration_secs`
//!   gives a zero-frame or zero-rate file), so the
//!   mode decision and the bars are unchanged.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{canvas, col, text, Color, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::AudioClip;

use crate::rune_sec::app_theme;
use crate::rune_sec::audio_panel::{health_word, probe_clip, ClipHealth};
use crate::State;

/// Height the caption row takes under the strip in
/// selected-clip mode (Phase I, section 17): the canvas
/// gives up this much of its height so canvas + caption
/// together keep the strip's usual footprint. The
/// reservation is exactly the caption column's own
/// height — its 2 pt gap plus one LABEL line, which
/// Rune resolves at size × 1.25 = 15 pt for the 12 pt
/// LABEL (`TextStyle::line_px`) — pinned by the
/// `caption_strip_reservation_matches_the_label_line`
/// test. (Phase I reserved 18 pt against an assumed
/// 16 pt line; the captioned strip actually totalled
/// one point SHORT of the bare strip until UI mission
/// Wave 9 re-proved the arithmetic from the constants.)
const CAPTION_STRIP_H: f32 = 17.0;
/// Span bracket geometry (Phase K, section 21), in
/// element points: the top rule's y, the edge ticks'
/// length downward from it, and the stroke width all
/// three bracket strokes share (between the centre
/// line's 1.0 and the playhead's 2.0).
const SPAN_BRACKET_Y: f32 = 2.0;
const SPAN_TICK_H: f32 = 6.0;
const SPAN_BRACKET_W: f32 = 1.5;
/// Alpha of the all-clips boundary ticks (Phase L,
/// section 36): the selection accent at a fainter
/// weight than the selected bracket's 0.75, so the
/// bracket stays the strong mark and the other clips'
/// edges read as quiet context under it.
const ALL_TICKS_ALPHA: f32 = 0.35;
/// Alpha multiplier for a muted clip's marks (Phase M,
/// section 16): the clip-mode bars and the selected
/// clip's span bracket keep their geometry and hue at
/// this fraction of their usual alpha — the strip's
/// visual "this clip makes no sound". (The value
/// matches [`ALL_TICKS_ALPHA`] by coincidence only;
/// the two constants mean different things and are
/// free to drift apart.)
const MUTED_BAR_ALPHA: f32 = 0.35;

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

/// The project frame at strip position `x` in
/// selected-clip mode (Phase J, section 31): the clip-span
/// counterpart of [`frame_at_x`]. The strip's left edge is
/// the clip's first frame and its right edge the clip's
/// last, so the offset is `floor(t * clip_len)` over the
/// clamped strip fraction — floor, not [`frame_at_x`]'s
/// round-to-nearest, because the right edge must land
/// exactly on the clip's final frame rather than rounding
/// past it. Edge semantics, pinned:
///
/// - `x = 0` reads as `clip_start`; `x = width` reads as
///   the last frame OF the clip, `clip_start + clip_len - 1`
///   (when `clip_len > 0`).
/// - `clip_len == 0` reads as `clip_start` at every `x`:
///   an empty clip has only its start to stand on.
/// - The result is clamped inside the project: a clip
///   whose span overruns the timeline's end (a start near
///   the end plus a long file) seeks no further than the
///   last project frame, and a degenerate project (no
///   frames) reads as frame 0. A stale `clip_start` past
///   the project's end loses to the project clamp — the
///   seek must land on a real frame.
/// - Degenerate strip inputs (non-positive or non-finite
///   width, non-finite `x`) read as `clip_start`, clamped
///   as above — the [`frame_at_x`] discipline, transposed.
pub fn clip_seek_frame(
    x: f32,
    width: f32,
    clip_start: usize,
    clip_len: usize,
    frame_count: usize,
) -> usize {
    if frame_count == 0 {
        return 0;
    }
    let last_project = frame_count - 1;
    if clip_len == 0 || width <= 0.0 || !width.is_finite() || !x.is_finite() {
        return clip_start.min(last_project);
    }
    let t = (x / width).clamp(0.0, 1.0);
    let offset = (t * clip_len as f32).floor() as usize;
    let frame = clip_start.saturating_add(offset);
    frame
        .min(clip_start.saturating_add(clip_len - 1))
        .min(last_project)
}

/// A clip's length in project frames, from its probed
/// duration in seconds: `(duration_secs * fps).round()`,
/// the conversion the engine itself uses when it schedules
/// clips (`src/editor.rs`, the split op's `dur_frames`).
/// Non-positive or non-finite inputs (an unreadable or
/// zero-length file, a hand-edited zero fps) read as 0 —
/// "length unknown", which sends the seek back to the
/// project mapping rather than inventing a span.
pub fn clip_len_frames(duration_secs: f32, fps: f32) -> usize {
    if !duration_secs.is_finite() || !fps.is_finite() || duration_secs <= 0.0 || fps <= 0.0 {
        return 0;
    }
    (duration_secs * fps).round() as usize
}

/// The x edges `(left, right)`, in element points, of a
/// clip span's bracket on a strip `width` points wide
/// over a project of `frame_count` frames (Phase K,
/// section 21): each frame owns an equal cell of the
/// strip, so the left edge is `start / frame_count *
/// width` and the right edge is `(start + len) /
/// frame_count * width`, both clamped into the strip —
/// a span running past the project's end (a start near
/// the end plus a long file) ends flush at the right
/// edge, and one starting past the end collapses onto
/// it. `None` when there is nothing to bracket: no
/// frames, a zero-length span, or a strip with no usable
/// width (non-positive or non-finite).
///
/// The bracket is drawn in project coordinates — where
/// the clip sits in the project — even in clip mode,
/// because the playhead it accompanies is also
/// project-mapped; that is the deliberate asymmetry
/// with [`clip_seek_frame`], whose clip-mode drag is
/// mapped in clip coordinates instead.
pub fn span_bracket(span: (usize, usize), frame_count: usize, width: f32) -> Option<(f32, f32)> {
    let (start, len) = span;
    if frame_count == 0 || len == 0 || width <= 0.0 || !width.is_finite() {
        return None;
    }
    let count = frame_count as f32;
    let left = (start as f32 / count * width).clamp(0.0, width);
    let end = start.saturating_add(len);
    let right = (end as f32 / count * width).clamp(0.0, width);
    Some((left, right))
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

/// Whether clip `index` is muted (Phase M, section 16):
/// the strip's one read of `AudioClip::muted`, shared by
/// the clip-mode bars/caption and the span bracket so
/// the two paths can never disagree about what "muted"
/// means here. An out-of-range index reads as `false` —
/// a stale selection is unmuted for drawing purposes,
/// like every other stale-selection read in this module.
pub fn clip_is_muted(core: &EditorCore, index: usize) -> bool {
    core.project
        .audio
        .get(index)
        .map(|clip| clip.muted)
        .unwrap_or(false)
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

/// The caption text for clip `index` (0-based; the
/// caption counts from 1, the audio panel's own
/// numbering — its op statuses count clips from 1):
/// `"Clip {n}"`, or `"Clip {n} · muted"` when the clip
/// is muted (Phase M, section 16). The unmuted string
/// is byte-identical to the caption Phase I introduced;
/// muting only appends the suffix.
pub fn clip_caption(index: usize, muted: bool) -> String {
    if muted {
        format!("Clip {} · muted", index + 1)
    } else {
        format!("Clip {}", index + 1)
    }
}

/// The caption naming the strip's mode, or `None` when no
/// caption element is built at all: [`clip_caption`] of
/// the selected clip in clip mode, and `None` in project
/// mode, whose element tree stays exactly what it was
/// before Phase I (section 17). `muted` is the mode's
/// clip's flag ([`clip_is_muted`]); it is ignored in
/// project mode, which grows no caption either way.
pub fn waveform_caption(mode: WaveformMode, muted: bool) -> Option<String> {
    match mode {
        WaveformMode::Clip(i) => Some(clip_caption(i, muted)),
        WaveformMode::Project => None,
    }
}

/// The health caption for the SELECTED clip (Wave 6-D;
/// see the module docs): `"Clip {n} · {health}"` with
/// the audio panel's own health vocabulary
/// ([`health_word`] — "missing file" / "unsupported
/// format" / "unreadable file") and its 1-based clip
/// numbering, or `None` for a healthy clip — a healthy
/// selection grows no health caption, exactly as a
/// healthy row grows no badge in the panel. The caller
/// (the builder's project-mode tail) only reaches this
/// with a valid selection whose peaks would not decode;
/// the health itself comes from the panel's cached
/// probe ([`probe_clip`]), never a second decoder.
pub fn clip_health_caption(index: usize, health: ClipHealth) -> Option<String> {
    health_word(health).map(|word| format!("Clip {} · {word}", index + 1))
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
fn waveform_bars(core: &EditorCore, el_size: [f32; 2], t: &Theme) -> Shape {
    let (w, h) = (el_size[0], el_size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let mid = h / 2.0;
    let centre = t.card_border;
    shape = shape.line([0.0, mid], [w, mid], 1.0, LineCap::Butt, centre);

    let Some(clip) = core.project.audio.last() else {
        return shape;
    };
    let buckets = strip_buckets(strip_columns(w));
    let Some((peaks, _)) = clip_peaks(clip, buckets) else {
        return shape;
    };
    // Project mode never dims its bars: they show the
    // last clip, selected or not, and Phase M dims only
    // the selected clip's marks (its clip-mode bars and
    // its span bracket) — hence `false` here, always.
    draw_peak_bars(shape, &peaks, clip.gain, false, t, w, h)
}

/// The bar colours `(outer, inner)` for one clip's
/// peaks: the theme accent
/// ([`app_theme::accent`]) at half
/// alpha around the opaque accent — the pair
/// [`draw_peak_bars`] has drawn since Phase D (the hue
/// itself moved from a file-local hex literal to the
/// theme's accent in UI mission Wave 2). A muted clip
/// keeps both hues at
/// [`MUTED_BAR_ALPHA`] of those alphas (Phase M,
/// section 16): `Color::alpha` SETS the component, so
/// the multiplication happens here, in the open, where
/// a test can pin it — the bars' geometry is untouched
/// either way.
fn bar_colors(t: &Theme, muted: bool) -> (Color, Color) {
    let accent = app_theme::accent(t);
    if muted {
        (
            accent.alpha(0.5 * MUTED_BAR_ALPHA),
            accent.alpha(MUTED_BAR_ALPHA),
        )
    } else {
        (accent.alpha(0.5), accent)
    }
}

/// Append the mirrored bar columns for `peaks` (reduced to
/// the strip's display columns, amplitudes scaled by
/// `gain` through [`bar_span`]) to `shape` — the drawing
/// tail [`waveform_bars`] and [`waveform_bars_clip`] share,
/// so project mode and selected-clip mode render peaks
/// identically; only the peaks' source differs. `muted`
/// dims the bar colours through [`bar_colors`] without
/// touching a single bar's geometry.
fn draw_peak_bars(
    shape: Shape,
    peaks: &[f32],
    gain: f32,
    muted: bool,
    t: &Theme,
    w: f32,
    h: f32,
) -> Shape {
    let mut shape = shape;
    let mid = h / 2.0;
    let columns = strip_columns(w);
    let cols = reduce_columns(peaks, columns);
    let (outer, accent) = bar_colors(t, muted);
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
/// [`selected_clip_peaks`], gain NOT applied there), scaled
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
    t: &Theme,
) -> Shape {
    let (w, h) = (el_size[0], el_size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let mid = h / 2.0;
    let centre = t.card_border;
    shape = shape.line([0.0, mid], [w, mid], 1.0, LineCap::Butt, centre);
    let Some(clip) = core.project.audio.get(index) else {
        return shape;
    };
    // A muted selected clip draws its bars dimmed
    // (Phase M, section 16) — the flag rides along with
    // the gain, straight from the clip itself.
    draw_peak_bars(shape, peaks, clip.gain, clip.muted, t, w, h)
}

/// The span bracket's colour: the theme accent
/// ([`app_theme::accent`], the bars' own hue) at
/// 0.75 — or at [`MUTED_BAR_ALPHA`] of that when the
/// bracketed clip is muted (Phase M, section 16), the
/// same dimming the clip-mode bars get, applied the
/// same way (the product computed here, then set via
/// `Color::alpha`, which does not multiply). Factored
/// out of [`with_span_bracket`] so a test can pin the
/// arithmetic.
fn bracket_color(t: &Theme, muted: bool) -> Color {
    let accent = app_theme::accent(t);
    if muted {
        accent.alpha(0.75 * MUTED_BAR_ALPHA)
    } else {
        accent.alpha(0.75)
    }
}

/// Append the selected clip's span bracket to a bars
/// shape (Phase K, section 21): a rule at
/// [`SPAN_BRACKET_Y`] across the span's x edges with a
/// [`SPAN_TICK_H`]-long tick down at each edge, in the
/// theme accent ([`bracket_color`]) at reduced alpha.
/// The span resolves through
/// [`clip_span`] at the strip's own bucket count — the
/// same cached probe, keyed the same way, that the seek
/// uses — and the edges come from [`span_bracket`],
/// project-mapped in both modes (see its docs for the
/// deliberate asymmetry with the clip-mode seek). The
/// caller appends the playhead after this, so the marker
/// stays on top. No resolvable span — no selection, a
/// stale selection, an unreadable or zero-length clip
/// file — leaves the shape untouched, so both modes'
/// strips are exactly what they were before this
/// section wherever the bracket does not apply.
fn with_span_bracket(
    shape: Shape,
    core: &EditorCore,
    selected: Option<usize>,
    el_size: [f32; 2],
    t: &Theme,
) -> Shape {
    let mut shape = shape;
    let (w, h) = (el_size[0], el_size[1]);
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let buckets = strip_buckets(strip_columns(w));
    let Some(span) = clip_span(core, selected, buckets) else {
        return shape;
    };
    let Some((left, right)) = span_bracket(span, core.frame_count(), w) else {
        return shape;
    };
    // The bracket belongs to the selected clip in both
    // modes; a muted selected clip dims it (Phase M,
    // section 16) exactly as it dims its clip-mode bars.
    // `span` resolved, so `selected` names a real clip —
    // the muted read still goes through the shared
    // helper, never the field directly.
    let muted = selected.map(|i| clip_is_muted(core, i)).unwrap_or(false);
    let bracket = bracket_color(t, muted);
    let y = SPAN_BRACKET_Y;
    let tick = y + SPAN_TICK_H;
    shape = shape.line([left, y], [right, y], SPAN_BRACKET_W, LineCap::Butt, bracket);
    shape = shape.line([left, y], [left, tick], SPAN_BRACKET_W, LineCap::Butt, bracket);
    shape = shape.line([right, y], [right, tick], SPAN_BRACKET_W, LineCap::Butt, bracket);
    shape
}

/// Append every clip's boundary ticks to a bars shape
/// (Phase L, section 36): a [`SPAN_TICK_H`]-long tick
/// down from [`SPAN_BRACKET_Y`] at each edge
/// [`all_clip_edges`] resolves — the bracket's tick
/// geometry at [`ALL_TICKS_ALPHA`], with no top rule,
/// so the selected clip's bracket (appended after this,
/// by the caller) still reads as the strong mark and
/// draws over its own clip's ticks. Project mode only:
/// the selected-clip shape never calls this (see the
/// module docs). No resolvable spans — no clips, or
/// every clip's file unreadable — leaves the shape
/// untouched.
fn with_all_clip_ticks(shape: Shape, core: &EditorCore, el_size: [f32; 2], t: &Theme) -> Shape {
    let mut shape = shape;
    let (w, h) = (el_size[0], el_size[1]);
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let buckets = strip_buckets(strip_columns(w));
    let edges = all_clip_edges(core, buckets, w);
    if edges.is_empty() {
        return shape;
    }
    let tick_color = app_theme::accent(t).alpha(ALL_TICKS_ALPHA);
    let y = SPAN_BRACKET_Y;
    let tick = y + SPAN_TICK_H;
    for x in edges {
        shape = shape.line([x, y], [x, tick], SPAN_BRACKET_W, LineCap::Butt, tick_color);
    }
    shape
}

/// The strip for [`waveform_el_state`]: the centre line and
/// bars of [`waveform_bars`], plus every clip's boundary
/// ticks ([`with_all_clip_ticks`], Phase L), the selected
/// clip's span bracket ([`with_span_bracket`], Phase K)
/// and the project-mapped playhead ([`with_playhead`]) —
/// in that draw order, so each stronger mark lands over
/// the quieter one.
fn waveform_shape_project(
    core: &EditorCore,
    selected: Option<usize>,
    el_size: [f32; 2],
    t: &Theme,
) -> Shape {
    with_playhead(
        with_span_bracket(
            with_all_clip_ticks(waveform_bars(core, el_size, t), core, el_size, t),
            core,
            selected,
            el_size,
            t,
        ),
        core,
        el_size,
        t,
    )
}

/// The playhead's colour: the warning amber
/// ([`app_theme::warning_on`] — the brief's warning
/// tint in dark, darkened for the white card in
/// light). Factored out so the colour has a named
/// home in this file (UI mission Wave 2; before that
/// the playhead drew a file-local amber literal, the
/// stage's selected-joint tone).
fn playhead_color(t: &Theme) -> Color {
    app_theme::warning_on(t)
}

/// Append the project-mapped playhead to a bars shape:
/// [`playhead_x`] of the current frame, drawn whenever the
/// project has frames — including over the empty lane of a
/// project with no (readable) clip, since the strip still
/// seeks there. Drawn last, over the bars, in the
/// warning amber ([`playhead_color`]). Shared by the
/// project shape and the selected-clip shape (Phase I,
/// section 17): the playhead is project-mapped in both
/// modes, deliberately (see the module docs).
fn with_playhead(shape: Shape, core: &EditorCore, el_size: [f32; 2], t: &Theme) -> Shape {
    let mut shape = shape;
    let (w, h) = (el_size[0], el_size[1]);
    if w > 0.0 && h > 0.0 && core.frame_count() > 0 {
        let x = playhead_x(core.current, core.frame_count(), w);
        let playhead = playhead_color(t);
        shape = shape.line([x, 0.0], [x, h], 2.0, LineCap::Butt, playhead);
    }
    shape
}

/// The selected-clip strip (Phase I, section 17):
/// [`waveform_bars_clip`] plus the same project-mapped
/// playhead as the project strip ([`with_playhead`]) and
/// the same span bracket ([`with_span_bracket`],
/// Phase K), here over the very clip the strip shows —
/// drawn even though the span is the strip's whole
/// subject, so it frames the mode visually.
fn waveform_shape_clip(
    core: &EditorCore,
    index: usize,
    peaks: &[f32],
    el_size: [f32; 2],
    t: &Theme,
) -> Shape {
    with_playhead(
        with_span_bracket(
            waveform_bars_clip(core, index, peaks, el_size, t),
            core,
            Some(index),
            el_size,
            t,
        ),
        core,
        el_size,
        t,
    )
}

/// The selected clip's span on the timeline — `(start,
/// length in frames)` — when the audio panel's selection
/// names a real clip and that clip's length is known
/// (Phase J, section 31). The length rides along with the
/// peaks cache: this module's own [`clip_peaks`] probe
/// returns the file's duration alongside its peaks and
/// caches the pair under the same key, so resolving a span
/// decodes an unchanged file at most once (on the first
/// press, at the strip's current bucket count); every
/// later press is a cache hit. An unreadable file probes
/// as `None` and reads as "span unknown" — failures are
/// never cached, so a file that appears later resolves
/// then. A zero-length file resolves to length 0, which
/// the caller treats as unknown: an empty clip has no
/// span to scrub within.
fn clip_span(core: &EditorCore, selected: Option<usize>, buckets: usize) -> Option<(usize, usize)> {
    let i = selected?;
    let clip = core.project.audio.get(i)?;
    let (_, duration_secs) = clip_peaks(clip, buckets)?;
    let len = clip_len_frames(duration_secs, core.project.fps);
    Some((clip.start_frame, len))
}

/// The x positions, in element points, of every audio
/// clip's span edges on a strip `width` points wide —
/// the project-mode boundary ticks' source (Phase L,
/// section 36). Each clip index resolves through the
/// very same path the selected clip's bracket uses —
/// [`clip_span`] at `buckets`, then [`span_bracket`]
/// over the project's frame count — so a tick always
/// stands exactly where a bracket edge for that clip
/// would. A clip contributes both its edges or none:
/// an unreadable file (span unknown) and a zero-length
/// span (no bracket) both skip, never invent. The
/// positions come back sorted ascending, ready to
/// draw; duplicates are kept — two clips sharing a
/// boundary frame each contribute their own edge
/// there, and a span collapsing onto the strip's right
/// edge contributes it twice — because the collection
/// is per-clip by construction, and the caller asked
/// for every clip's edges, not a merged set.
pub fn all_clip_edges(core: &EditorCore, buckets: usize, width: f32) -> Vec<f32> {
    let frame_count = core.frame_count();
    let mut edges = Vec::new();
    for i in 0..core.project.audio.len() {
        let Some(span) = clip_span(core, Some(i), buckets) else {
            continue;
        };
        if let Some((left, right)) = span_bracket(span, frame_count, width) {
            edges.push(left);
            edges.push(right);
        }
    }
    edges.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    edges
}

/// The seek gesture for [`waveform_el_state`]: pressing —
/// and, holding, dragging — on the strip goes to the frame
/// under the pointer (click-to-seek plus scrub, one gesture).
/// Navigation only: `EditorCore::goto` moves the playhead
/// without a snapshot and posts no status, so a scrub leaves
/// the undo stack and the status line exactly as they were
/// (the timeline chips' click semantics). The frame comes
/// from the element's own laid-out width (`Drag::w`, the
/// same local geometry the drag reports), so the seek
/// follows the strip as actually drawn; a gesture on a
/// zero-width element is ignored. `Phase::Up` does
/// nothing — the last Down/Move position stands — and a
/// cancelled gesture has nothing to restore, for the same
/// reason: goto never mutated anything but the playhead.
///
/// Which mapping (Phase J, section 31): when the strip is
/// in selected-clip mode and the selected clip's span is
/// known and non-empty, the drag seeks WITHIN that span
/// via [`clip_seek_frame`] — the bars on screen are that
/// clip's own shape, so the pointer scrubs the clip it is
/// looking at. In every other case — project mode, a stale
/// selection, an unreadable or zero-length clip file —
/// the frame comes from [`frame_at_x`] over the whole
/// timeline, exactly the pre-Phase-J behaviour. Only the
/// drag's mapping differs; the playhead's DRAWING stays
/// project-mapped in both modes ([`with_playhead`]), so
/// the marker keeps agreeing with the timeline chips and
/// the stage: the frame a clip-mode drag lands on is an
/// ordinary project frame inside the clip's span, and it
/// is drawn the way every other surface draws it.
fn waveform_seek(s: &mut State, d: &Drag) {
    match d.phase {
        Phase::Down | Phase::Move => {
            if d.w > 0.0 {
                let buckets = strip_buckets(strip_columns(d.w));
                let frame = match clip_span(&s.core, s.audio_panel.selected, buckets) {
                    Some((start, len)) if len > 0 => {
                        clip_seek_frame(d.x, d.w, start, len, s.core.frame_count())
                    }
                    _ => frame_at_x(d.x, d.w, s.core.frame_count()),
                };
                s.core.goto(frame);
            }
        }
        Phase::Up => {}
    }
}

/// The selected clip's peaks at `buckets`, for
/// [`waveform_el_state`]'s mode decision and clip-mode
/// bars (Wave 10; see the module docs' Wave 10 section):
/// the selected clip's own peaks, or an empty vec —
/// "no peaks", the project-mode signal — for no
/// selection, a stale selection, or a clip whose file
/// yields no peaks.
///
/// The probe goes through this module's cached
/// [`clip_peaks`], NOT the library's
/// `audio_ops::clip_peaks` it used through Phase I:
/// that probe decodes the clip's whole WAV on every
/// call, and the builder calls this on every build
/// whenever a clip is selected — during playback, every
/// frame. The cached probe decodes an unchanged file
/// once and serves a deleted file stale, the project
/// path's Phase E behaviour since section 27.
///
/// One library semantic is preserved deliberately: the
/// library probe returns no peaks for a file that
/// decodes to no sample frames, while the cached probe
/// returns that file's all-zero buckets (non-empty —
/// it would read as Clip mode). Zero content is the
/// one probe whose duration reads exactly 0.0, so the
/// duration half of the cached result carries the
/// carve-out; a genuinely silent clip (real frames,
/// real duration, zero peaks) still reads as Clip
/// mode, as [`waveform_mode`] documents.
fn selected_clip_peaks(core: &EditorCore, selected: Option<usize>, buckets: usize) -> Vec<f32> {
    let Some(clip) = selected.and_then(|i| core.project.audio.get(i)) else {
        return Vec::new();
    };
    match clip_peaks(clip, buckets) {
        Some((peaks, duration_secs)) if duration_secs > 0.0 => peaks,
        _ => Vec::new(),
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
/// what it was before this section. The playhead is
/// project-mapped in BOTH modes, deliberately (see the
/// module docs); the seek gesture is project-mapped in
/// project mode and — since Phase J (section 31) —
/// clip-span-mapped in clip mode when the selected
/// clip's length is known (see [`waveform_seek`]).
/// Phase K (section 21) adds the selected clip's span
/// bracket to both shapes (see the module docs): no new
/// element, no height or hit-area change — the canvas
/// and the caption are exactly what they were.
/// Phase L (section 36) adds every clip's boundary
/// ticks to the PROJECT shape only, under the bracket
/// (see the module docs): again no new element, and
/// the clip shape, the seek and the playhead are
/// exactly what they were.
/// Phase M (section 16) makes a muted clip look muted
/// (see the module docs): dimmer bars and a "· muted"
/// caption suffix in clip mode, a dimmer span bracket
/// in both modes — colours and one string only, no
/// element, geometry or gesture changes.
/// UI mission Wave 2 (s02 §3.1 / s14 D7) threads the
/// shell's `&Theme` through this builder and the shape
/// builders under it: every colour the strip draws now
/// derives from the token architecture (see the module
/// docs' "Theme wiring" section) instead of file-local
/// hex constants. The element tree, geometry and
/// gesture are exactly what they were.
/// UI mission Wave 6-D adds one project-mode branch:
/// a selected clip the panel's probe finds non-healthy
/// (missing / unsupported / unreadable — the states
/// whose peaks cannot decode, hence the project
/// fallback) returns the strip in a column with a
/// warning-ink health caption in the caption slot
/// (see the module docs' "Clip health caption").
/// Every other input — healthy selection, stale
/// selection, no selection — returns exactly the tree
/// it returned before.
/// UI mission Wave 9 pins the height accounting the
/// phases above assumed: the bare return now sizes
/// its canvas (`.h(el_size[1])` — an unsized canvas
/// leaf lays out 0 pt high in Rune's layout, text
/// being the only measured leaf), and
/// [`CAPTION_STRIP_H`] is the caption column's true
/// height (2 pt gap + one 15 pt LABEL line), so all
/// three returns occupy exactly `el_size[1]` —
/// before, the captioned columns totalled one point
/// less and the bare canvas had no layout height
/// at all.
/// Wave 10 reroutes the selected-clip probe under
/// this builder through [`selected_clip_peaks`] (this
/// module's peaks cache) instead of the library's
/// decode-on-every-call probe — same peaks, same mode
/// decision, no per-build WAV decode (see the module
/// docs' Wave 10 section). The element tree, geometry
/// and gesture are exactly what they were.
pub fn waveform_el_state(s: &State, el_size: [f32; 2], t: &Theme) -> El {
    let clip_count = s.core.project.audio.len();
    let selected_peaks: Vec<f32> = selected_clip_peaks(
        &s.core,
        s.audio_panel.selected,
        strip_buckets(strip_columns(el_size[0])),
    );
    let mode = waveform_mode(s.audio_panel.selected, clip_count, &selected_peaks);
    let muted = match mode {
        WaveformMode::Clip(i) => clip_is_muted(&s.core, i),
        WaveformMode::Project => false,
    };
    if let (WaveformMode::Clip(i), Some(caption)) = (mode, waveform_caption(mode, muted)) {
        let canvas_h = (el_size[1] - CAPTION_STRIP_H).max(0.0);
        return col()
            .gap(2.0)
            .child(
                canvas(waveform_shape_clip(
                    &s.core,
                    i,
                    &selected_peaks,
                    [el_size[0], canvas_h],
                    t,
                ))
                .key("waveform")
                .h(canvas_h)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| waveform_seek(s, d)),
            )
            .child(
                text(caption)
                    .key("waveform-caption")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
    }
    // Wave 6-D (see the module docs' "Clip health
    // caption"): project mode with a valid selection
    // whose file the panel's probe finds non-healthy
    // gets the health caption in the caption slot. The
    // probe is the panel's own, served from the panel
    // state's cache — no second decode — and a healthy
    // probe falls through to the bare canvas below, so
    // the healthy / stale / unselected trees (and their
    // geometry) are byte-identical to before.
    if let Some(i) = s.audio_panel.selected.filter(|&i| i < clip_count) {
        let probe = probe_clip(&s.audio_panel.probe_cache, &s.core.project.audio[i]);
        if let Some(note) = clip_health_caption(i, probe.health) {
            let canvas_h = (el_size[1] - CAPTION_STRIP_H).max(0.0);
            return col()
                .gap(2.0)
                .child(
                    canvas(waveform_shape_project(
                        &s.core,
                        s.audio_panel.selected,
                        [el_size[0], canvas_h],
                        t,
                    ))
                    .key("waveform")
                    .h(canvas_h)
                    .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| waveform_seek(s, d)),
                )
                .child(
                    text(note)
                        .key("waveform-caption")
                        .font(type_::LABEL)
                        .color(app_theme::warning_on(t)),
                );
        }
    }
    // The bare canvas carries its height explicitly
    // (UI mission Wave 9): Rune's layout measures text
    // leaves only, so a canvas with no `.h(..)` — this
    // return, alone of the builder's three — laid out
    // 0 pt high and let its shape paint its natural
    // 56 pt past the card's bottom edge, while both
    // captioned returns above size theirs. Same key,
    // same gesture; the box is now the `el_size` the
    // shape is built for.
    canvas(waveform_shape_project(&s.core, s.audio_panel.selected, el_size, t))
        .key("waveform")
        .h(el_size[1])
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
            muted: false,
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

    // ---- Selected-clip probe (Wave 10) ----

    #[test]
    fn selected_clip_peaks_match_the_library_probe() {
        // The rerouted probe answers the question the
        // library probe answered, with the same values:
        // both reduce the same decode with the same
        // `audio_edit::peaks`, so the vecs are identical.
        let path = temp_wav_path("selected-values");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let mut core = EditorCore::new();
        core.project.audio.push(clip_at(&path));
        let via_helper = selected_clip_peaks(&core, Some(0), 64);
        let via_library = stykanimate_rs::audio_ops::clip_peaks(&core, 0, 64);
        assert_eq!(via_helper.len(), 64);
        assert_eq!(via_helper, via_library);
        assert!(via_helper.iter().all(|&v| v > 0.7 && v <= 0.81));
        // No selection and a stale selection both read
        // as "no peaks", as the library path did.
        assert!(selected_clip_peaks(&core, None, 64).is_empty());
        assert!(selected_clip_peaks(&core, Some(4), 64).is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn selected_clip_peaks_survive_file_deletion_via_the_cache() {
        // The Wave 10 defect, pinned shut: the builder's
        // old probe decoded on every call, so deleting
        // the file mid-session flipped the strip back to
        // project mode on the next build. The rerouted
        // probe serves the cached peaks instead — the
        // project path's Phase E stale behaviour.
        let path = temp_wav_path("selected-cache");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let mut core = EditorCore::new();
        core.project.audio.push(clip_at(&path));
        let first = selected_clip_peaks(&core, Some(0), 64);
        assert_eq!(first.len(), 64);
        std::fs::remove_file(&path).unwrap();
        // The library probe now finds nothing — so an
        // identical second result can only have come
        // from this module's cache, never a re-decode.
        assert!(stykanimate_rs::audio_ops::clip_peaks(&core, 0, 64).is_empty());
        let second = selected_clip_peaks(&core, Some(0), 64);
        assert_eq!(first, second);
    }

    #[test]
    fn selected_clip_peaks_zero_content_file_has_no_peaks() {
        // A WAV that decodes to no sample frames: the
        // cached probe alone would report its all-zero
        // buckets (non-empty), but the library probe this
        // replaces reported none — the duration carve-out
        // in `selected_clip_peaks` keeps that reading, so
        // the mode decision still falls back to Project.
        let path = temp_wav_path("selected-empty");
        let silent = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: Vec::new(),
        };
        std::fs::write(&path, write_wav(&silent)).unwrap();
        let c = clip_at(&path);
        // The cached probe's raw answer, pinned so the
        // carve-out's reason stays visible: all-zero
        // peaks and a duration of exactly 0.0.
        let (peaks, duration) = clip_peaks(&c, 64).expect("zero-content probe");
        assert_eq!(peaks.len(), 64);
        assert!(peaks.iter().all(|&v| v == 0.0));
        assert_eq!(duration, 0.0);
        // The builder's probe reads it as no peaks.
        let mut core = EditorCore::new();
        core.project.audio.push(c);
        assert!(selected_clip_peaks(&core, Some(0), 64).is_empty());
        assert_eq!(
            waveform_mode(Some(0), 1, &selected_clip_peaks(&core, Some(0), 64)),
            WaveformMode::Project
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn selected_clip_peaks_missing_file_has_no_peaks() {
        // The ordinary fallback, unchanged by the
        // reroute: an unprobeable selected clip reads as
        // no peaks (failures are never cached, so a file
        // that appears later still probes fresh through
        // the builder's probe too).
        let path = temp_wav_path("selected-missing");
        let _ = std::fs::remove_file(&path);
        let mut core = EditorCore::new();
        core.project.audio.push(clip_at(&path));
        assert!(selected_clip_peaks(&core, Some(0), 64).is_empty());
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        assert_eq!(selected_clip_peaks(&core, Some(0), 64).len(), 64);
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
            waveform_caption(WaveformMode::Clip(0), false).as_deref(),
            Some("Clip 1")
        );
        assert_eq!(
            waveform_caption(WaveformMode::Clip(4), false).as_deref(),
            Some("Clip 5")
        );
        // Project mode grows no caption element at all.
        assert_eq!(waveform_caption(WaveformMode::Project, false), None);
    }

    // ---- Clip health caption (Wave 6-D) ----

    #[test]
    fn clip_health_caption_names_the_three_failure_states() {
        // The panel's vocabulary, 1-based numbering.
        assert_eq!(
            clip_health_caption(0, ClipHealth::Missing).as_deref(),
            Some("Clip 1 · missing file")
        );
        assert_eq!(
            clip_health_caption(2, ClipHealth::Unsupported).as_deref(),
            Some("Clip 3 · unsupported format")
        );
        assert_eq!(
            clip_health_caption(1, ClipHealth::Unreadable).as_deref(),
            Some("Clip 2 · unreadable file")
        );
        // A healthy clip grows no health caption.
        assert_eq!(clip_health_caption(0, ClipHealth::Healthy), None);
    }

    #[test]
    fn caption_strip_reservation_matches_the_label_line() {
        // UI mission Wave 9: the caption column under
        // the canvas is the column's 2 pt gap plus one
        // LABEL line, and Rune resolves a text style's
        // line height as size x 1.25 when none is set
        // (`TextStyle::line_px`) — 15 pt for the 12 pt
        // LABEL. CAPTION_STRIP_H must equal that sum
        // exactly: the canvas gives up CAPTION_STRIP_H
        // and the column adds the gap and the line back,
        // so any drift makes a captioned strip's total
        // height differ from the bare strip's `el_size`
        // height by the same amount.
        assert_eq!(type_::LABEL.line_px(), 15.0);
        assert_eq!(CAPTION_STRIP_H, 2.0 + type_::LABEL.line_px());
    }

    #[test]
    fn waveform_builds_with_a_missing_selected_clip() {
        // The health-caption branch end to end: the
        // selected clip's file does not exist, so its
        // peaks cannot decode, the mode falls back to
        // Project, and the builder takes the caption
        // column instead of the bare canvas.
        let path = temp_wav_path("health-missing");
        let _ = std::fs::remove_file(&path);
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.audio_panel.selected = Some(0);
        let t = rune::tok::theme(true);
        let _captioned = waveform_el_state(&s, [308.0, 56.0], &t);
        // The probe the builder consulted agrees.
        let probe = probe_clip(&s.audio_panel.probe_cache, &s.core.project.audio[0]);
        assert_eq!(probe.health, ClipHealth::Missing);
    }

    #[test]
    fn waveform_builds_with_an_unsupported_selected_clip() {
        // A real file with a non-WAV extension: the
        // probe calls it Unsupported without decoding.
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-waveform-{}-health.mp3",
            std::process::id()
        ));
        std::fs::write(&path, b"not a wav, whatever the bytes").unwrap();
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.audio_panel.selected = Some(0);
        let t = rune::tok::theme(true);
        let _captioned = waveform_el_state(&s, [308.0, 56.0], &t);
        let probe = probe_clip(&s.audio_panel.probe_cache, &s.core.project.audio[0]);
        assert_eq!(probe.health, ClipHealth::Unsupported);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn waveform_builds_with_an_unreadable_selected_clip() {
        // A `.wav` path whose bytes the decoder
        // rejects: Unreadable, same caption branch.
        let path = temp_wav_path("health-unreadable");
        std::fs::write(&path, b"this is not a wav file at all").unwrap();
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.audio_panel.selected = Some(0);
        let t = rune::tok::theme(true);
        let _captioned = waveform_el_state(&s, [308.0, 56.0], &t);
        let probe = probe_clip(&s.audio_panel.probe_cache, &s.core.project.audio[0]);
        assert_eq!(probe.health, ClipHealth::Unreadable);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn waveform_builds_with_a_healthy_selected_clip() {
        // The guard on the health branch: a healthy
        // selected clip probes Healthy, so no health
        // caption is produced and the builder takes the
        // pre-existing clip-mode tree.
        let path = temp_wav_path("health-healthy");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.audio_panel.selected = Some(0);
        let probe = probe_clip(&s.audio_panel.probe_cache, &s.core.project.audio[0]);
        assert_eq!(probe.health, ClipHealth::Healthy);
        assert_eq!(clip_health_caption(0, probe.health), None);
        let t = rune::tok::theme(true);
        let _clip_mode = waveform_el_state(&s, [308.0, 56.0], &t);
        let _ = std::fs::remove_file(&path);
    }

    // ---- Muted clips (Phase M, section 16) ----

    #[test]
    fn clip_is_muted_reads_the_flag() {
        let mut core = EditorCore::new();
        // No clips at all: every index is out of range.
        assert!(!clip_is_muted(&core, 0));
        assert!(!clip_is_muted(&core, 9));
        core.project.audio.push(AudioClip {
            path: "/nonexistent/a.wav".to_string(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(AudioClip {
            path: "/nonexistent/b.wav".to_string(),
            start_frame: 4,
            gain: 1.0,
            muted: true,
        });
        assert!(!clip_is_muted(&core, 0));
        assert!(clip_is_muted(&core, 1));
        // Past the end reads as unmuted, never a panic.
        assert!(!clip_is_muted(&core, 2));
    }

    #[test]
    fn clip_caption_muted_suffix() {
        // Unmuted: byte-identical to Phase I's caption.
        assert_eq!(clip_caption(0, false), "Clip 1");
        assert_eq!(clip_caption(4, false), "Clip 5");
        // Muted: the suffix is appended, nothing else moves.
        assert_eq!(clip_caption(0, true), "Clip 1 · muted");
        assert_eq!(clip_caption(4, true), "Clip 5 · muted");
        // The mode-level caption routes through the same
        // string, and project mode stays captionless even
        // when handed a muted flag.
        assert_eq!(
            waveform_caption(WaveformMode::Clip(0), true).as_deref(),
            Some("Clip 1 · muted")
        );
        assert_eq!(waveform_caption(WaveformMode::Project, true), None);
    }

    #[test]
    fn bar_colors_dim_when_muted() {
        let t = rune::tok::theme(true);
        // Unmuted is the pair the strip has always drawn:
        // outer at half alpha, inner opaque.
        let (outer, inner) = bar_colors(&t, false);
        assert_eq!(outer.a, 0.5);
        assert_eq!(inner.a, 1.0);
        // Muted: the SAME hues at MUTED_BAR_ALPHA of those
        // alphas — the multiplication Color::alpha does
        // not do, pinned here.
        let (m_outer, m_inner) = bar_colors(&t, true);
        assert_eq!(m_outer.a, 0.5 * MUTED_BAR_ALPHA);
        assert_eq!(m_inner.a, MUTED_BAR_ALPHA);
        assert_eq!((m_outer.r, m_outer.g, m_outer.b), (outer.r, outer.g, outer.b));
        assert_eq!((m_inner.r, m_inner.g, m_inner.b), (inner.r, inner.g, inner.b));
    }

    #[test]
    fn bracket_color_dims_when_muted() {
        let t = rune::tok::theme(true);
        // Unmuted: the Phase K bracket weight.
        assert_eq!(bracket_color(&t, false).a, 0.75);
        // Muted: the same factor as the bars.
        assert_eq!(bracket_color(&t, true).a, 0.75 * MUTED_BAR_ALPHA);
        // Same hue, only quieter.
        let (a, b) = (bracket_color(&t, false), bracket_color(&t, true));
        assert_eq!((a.r, a.g, a.b), (b.r, b.g, b.b));
    }

    #[test]
    fn waveform_builds_with_a_muted_selected_clip() {
        // End-to-end smoke over the muted paths: a muted
        // clip with a readable file, selected — the
        // builder takes clip mode (dimmed bars, suffixed
        // caption), and the project shape builds with the
        // dimmed bracket over it.
        let path = temp_wav_path("muted-build");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 3,
            gain: 1.0,
            muted: true,
        });
        s.audio_panel.selected = Some(0);
        assert!(clip_is_muted(&s.core, 0));
        let t = rune::tok::theme(true);
        let _clip_mode = waveform_el_state(&s, [308.0, 56.0], &t);
        let _project_shape = waveform_shape_project(&s.core, Some(0), [308.0, 56.0], &t);
        let _ = std::fs::remove_file(&path);
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

    // ---- Clip-mode seek (Phase J, section 31) ----

    #[test]
    fn clip_seek_frame_edges_and_interior() {
        // Clip at frames 10..=29 (start 10, length 20) in a
        // 100-frame project, strip 200 pt wide.
        let seek = |x: f32| clip_seek_frame(x, 200.0, 10, 20, 100);
        // Left edge is the clip's first frame, exactly.
        assert_eq!(seek(0.0), 10);
        // Interior: floor over the span — one frame per
        // 10 pt, no rounding up into the next frame.
        assert_eq!(seek(20.0), 12);
        assert_eq!(seek(99.0), 19);
        assert_eq!(seek(100.0), 20);
        // The last sliver and the right edge both land on
        // the clip's LAST frame, never one past it.
        assert_eq!(seek(199.0), 29);
        assert_eq!(seek(200.0), 29);
        // Outside the strip clamps to the clip's ends.
        assert_eq!(seek(-40.0), 10);
        assert_eq!(seek(9_999.0), 29);
    }

    #[test]
    fn clip_seek_frame_degenerate_spans() {
        // A zero-length clip has only its start to stand on.
        assert_eq!(clip_seek_frame(0.0, 200.0, 10, 0, 100), 10);
        assert_eq!(clip_seek_frame(200.0, 200.0, 10, 0, 100), 10);
        // A one-frame clip likewise.
        assert_eq!(clip_seek_frame(200.0, 200.0, 10, 1, 100), 10);
        // Degenerate strip inputs read as the clip's start.
        assert_eq!(clip_seek_frame(100.0, 0.0, 10, 20, 100), 10);
        assert_eq!(clip_seek_frame(f32::NAN, 200.0, 10, 20, 100), 10);
        // No project frames at all: frame 0, the only
        // reading that can never be out of range.
        assert_eq!(clip_seek_frame(100.0, 200.0, 10, 20, 0), 0);
        // A clip overrunning the project's end: the project
        // clamp wins over the clip's own last frame (109).
        assert_eq!(clip_seek_frame(0.0, 200.0, 90, 20, 100), 90);
        assert_eq!(clip_seek_frame(80.0, 200.0, 90, 20, 100), 98);
        assert_eq!(clip_seek_frame(100.0, 200.0, 90, 20, 100), 99);
        assert_eq!(clip_seek_frame(200.0, 200.0, 90, 20, 100), 99);
        // A stale start past the project's end loses to the
        // project clamp even at the strip's left edge.
        assert_eq!(clip_seek_frame(0.0, 200.0, 150, 20, 100), 99);
    }

    #[test]
    fn clip_len_frames_conversion() {
        // The engine's own conversion: seconds x fps,
        // rounded (durations at 12 fps, the demo rate).
        assert_eq!(clip_len_frames(1.0, 12.0), 12);
        assert_eq!(clip_len_frames(0.5, 12.0), 6);
        assert_eq!(clip_len_frames(2.5, 10.0), 25);
        assert_eq!(clip_len_frames(0.26, 12.0), 3);
        assert_eq!(clip_len_frames(0.3, 12.0), 4);
        // Unknown lengths read as 0, never as an invented
        // span: silent files, bad fps, non-finite probes.
        assert_eq!(clip_len_frames(0.0, 12.0), 0);
        assert_eq!(clip_len_frames(-1.0, 12.0), 0);
        assert_eq!(clip_len_frames(1.0, 0.0), 0);
        assert_eq!(clip_len_frames(f32::NAN, 12.0), 0);
        assert_eq!(clip_len_frames(1.0, f32::NAN), 0);
        assert_eq!(clip_len_frames(f32::INFINITY, 12.0), 0);
    }

    #[test]
    fn clip_span_resolution() {
        let mut core = EditorCore::new();
        // No selection, and a selection with no clips at
        // all: no span either way.
        assert_eq!(clip_span(&core, None, 96), None);
        assert_eq!(clip_span(&core, Some(0), 96), None);
        // One second of sine on disk, placed at frame 3:
        // the demo project runs at 12 fps, so the span is
        // (3, 12) — the duration rode along with the peaks
        // probe, no second decode path involved.
        let path = temp_wav_path("span");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 3,
            gain: 1.0,
            muted: false,
        });
        assert_eq!(clip_span(&core, Some(0), 96), Some((3, 12)));
        // A stale selection resolves to nothing.
        assert_eq!(clip_span(&core, Some(5), 96), None);
        // A clip whose file does not exist: length unknown,
        // so no span — the seek falls back to the project
        // mapping rather than guessing.
        core.project.audio.push(AudioClip {
            path: "/nonexistent/no-such-clip.wav".to_string(),
            start_frame: 7,
            gain: 1.0,
            muted: false,
        });
        assert_eq!(clip_span(&core, Some(1), 96), None);
        let _ = std::fs::remove_file(&path);
    }

    // ---- Span bracket (Phase K, section 21) ----

    #[test]
    fn span_bracket_edges() {
        // 16 frames over 256 pt: one frame cell is 16 pt,
        // so every edge below is exact.
        // A span at the project's start.
        assert_eq!(span_bracket((0, 4), 16, 256.0), Some((0.0, 64.0)));
        // A span in the middle.
        assert_eq!(span_bracket((4, 4), 16, 256.0), Some((64.0, 128.0)));
        // A span ending flush with the project's last frame.
        assert_eq!(span_bracket((12, 4), 16, 256.0), Some((192.0, 256.0)));
        // The whole project, edge to edge.
        assert_eq!(span_bracket((0, 16), 16, 256.0), Some((0.0, 256.0)));
        // A span running past the end (a start near the end
        // plus a long file): the right edge clamps to the
        // strip's right edge, never beyond it.
        assert_eq!(span_bracket((14, 8), 16, 256.0), Some((224.0, 256.0)));
        // A span starting past the end collapses onto the
        // right edge rather than drawing outside the strip.
        assert_eq!(span_bracket((20, 4), 16, 256.0), Some((256.0, 256.0)));
    }

    #[test]
    fn span_bracket_degenerate_inputs() {
        // No frames: no project to bracket against.
        assert_eq!(span_bracket((0, 4), 0, 256.0), None);
        // A zero-length span has no extent to bracket.
        assert_eq!(span_bracket((3, 0), 16, 256.0), None);
        // No usable strip width: zero, negative, non-finite.
        assert_eq!(span_bracket((0, 4), 16, 0.0), None);
        assert_eq!(span_bracket((0, 4), 16, -8.0), None);
        assert_eq!(span_bracket((0, 4), 16, f32::NAN), None);
        assert_eq!(span_bracket((0, 4), 16, f32::INFINITY), None);
    }

    #[test]
    fn waveform_builds_in_both_modes_with_a_selected_clip() {
        // One second of sine on disk at frame 3, selected:
        // the span resolves through the cached probe, and
        // the strip builds in clip mode (the bracket drawn
        // over the clip's own strip) and, with the
        // selection cleared, in project mode too.
        let path = temp_wav_path("bracket-build");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        let mut s = crate::init();
        s.core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: 3,
            gain: 1.0,
            muted: false,
        });
        s.audio_panel.selected = Some(0);
        // The span the bracket draws: 1 s at the demo
        // project's 12 fps, from frame 3.
        let buckets = strip_buckets(strip_columns(308.0));
        assert_eq!(clip_span(&s.core, Some(0), buckets), Some((3, 12)));
        // The mode gate agrees this is clip mode, and the
        // builder builds it, bracket included.
        let peaks = stykanimate_rs::audio_ops::clip_peaks(&s.core, 0, buckets);
        assert_eq!(
            waveform_mode(s.audio_panel.selected, 1, &peaks),
            WaveformMode::Clip(0)
        );
        let t = rune::tok::theme(true);
        let _clip_mode = waveform_el_state(&s, [308.0, 56.0], &t);
        // The project shape takes the same bracket path:
        // built directly with the resolving selection,
        // because the builder's own mode gate routes a
        // readable selection to clip mode (above).
        let _project_shape = waveform_shape_project(&s.core, Some(0), [308.0, 56.0], &t);
        // Project mode through the builder: no selection,
        // the bare canvas, no bracket.
        s.audio_panel.selected = None;
        let _project_mode = waveform_el_state(&s, [308.0, 56.0], &t);
        let _ = std::fs::remove_file(&path);
    }

    // ---- All-clip boundary ticks (Phase L, section 36) ----

    /// A core whose timeline runs to exactly 24 frames
    /// (the demo's handful, extended with `add_frame`),
    /// so the edge arithmetic in these tests lands on
    /// whole points: at width 240 each frame cell is
    /// exactly 10 pt.
    fn core_with_24_frames() -> EditorCore {
        let mut core = EditorCore::new();
        while core.frame_count() < 24 {
            core.add_frame();
        }
        assert_eq!(core.frame_count(), 24);
        core
    }

    fn plant_clip(core: &mut EditorCore, path: &std::path::Path, start_frame: usize) {
        core.project.audio.push(AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame,
            gain: 1.0,
            muted: false,
        });
    }

    #[test]
    fn all_clip_edges_two_clips_four_edges() {
        // Two 1-second clips on disk: at the demo
        // project's 12 fps each spans 12 frames. Over a
        // 24-frame project at width 240 (10 pt cells):
        // clip A at frame 0 spans (0, 12) -> edges 0 and
        // 120; clip B at frame 6 spans (6, 12) -> edges
        // 60 and 180. Collected and sorted: four edges.
        let mut core = core_with_24_frames();
        let path_a = temp_wav_path("edges-a");
        let path_b = temp_wav_path("edges-b");
        std::fs::write(&path_a, write_wav(&sine_wav())).unwrap();
        std::fs::write(&path_b, write_wav(&sine_wav())).unwrap();
        plant_clip(&mut core, &path_a, 0);
        plant_clip(&mut core, &path_b, 6);
        assert_eq!(
            all_clip_edges(&core, 96, 240.0),
            vec![0.0, 60.0, 120.0, 180.0]
        );
        // The project shape builds with the ticks drawn
        // (bars, ticks, no selection so no bracket,
        // playhead) — a smoke pass over the drawing path.
        let t = rune::tok::theme(true);
        let _shape = waveform_shape_project(&core, None, [240.0, 56.0], &t);
        let _ = std::fs::remove_file(&path_a);
        let _ = std::fs::remove_file(&path_b);
    }

    #[test]
    fn all_clip_edges_no_clips_is_empty() {
        // The demo project carries no audio: no spans,
        // no edges, whatever the strip width.
        let core = EditorCore::new();
        assert!(all_clip_edges(&core, 96, 240.0).is_empty());
        assert!(all_clip_edges(&core, 96, 308.0).is_empty());
    }

    #[test]
    fn all_clip_edges_skips_unreadable_files() {
        // One readable clip and one whose file does not
        // exist: the missing clip's span is unknown, so
        // it contributes no edges — only the real clip's
        // two edges stand.
        let mut core = core_with_24_frames();
        let path = temp_wav_path("edges-good");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        plant_clip(&mut core, &path, 0);
        core.project.audio.push(AudioClip {
            path: "/nonexistent/no-such-clip.wav".to_string(),
            start_frame: 7,
            gain: 1.0,
            muted: false,
        });
        assert_eq!(all_clip_edges(&core, 96, 240.0), vec![0.0, 120.0]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn all_clip_edges_degenerate_width_is_empty() {
        // A strip with no usable width brackets nothing,
        // so it ticks nothing either — the span_bracket
        // gate, inherited per clip.
        let mut core = core_with_24_frames();
        let path = temp_wav_path("edges-width");
        std::fs::write(&path, write_wav(&sine_wav())).unwrap();
        plant_clip(&mut core, &path, 0);
        assert!(all_clip_edges(&core, 96, 0.0).is_empty());
        assert!(all_clip_edges(&core, 96, -8.0).is_empty());
        assert!(all_clip_edges(&core, 96, f32::NAN).is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
