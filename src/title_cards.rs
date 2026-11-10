//! Chapter title cards (Roadmap Wave 2, W2-15 — the
//! P-15 follow-through W2-11's notes pointed at: "a title
//! card's natural anchor is `chapter.start_frame`").
//!
//! A title card is the episode furniture the harness's
//! films open each chapter with (Ultimate Harness v7:
//! put `"chapter"` on a chapter's first beat and "the
//! film shows a title card"): the chapter's number and
//! its verbatim title, centred on a translucent plate,
//! held over the chapter's first frames.
//!
//! ## The shape the architecture honestly supports
//!
//! Two carriers were on the table:
//!
//! * **(b) Inserted title frames** — splice held frames
//!   into the timeline at each chapter start. NOT built:
//!   inserting frames shifts every range citizen
//!   (cues, chapters, camera), and whether frame ops
//!   remap those ranges is an explicitly open question
//!   (IA-W2-11-4) this seat does not fork. A card that
//!   rewrites the user's timeline to exist is the wrong
//!   default.
//! * **(a) A burn-in overlay** — the subtitle funnel's
//!   own shape: a pure per-frame overlay derived from
//!   project data, composited at the raster funnels'
//!   tails, costing the timeline nothing and removable
//!   by the settings toggle. THIS is what is built.
//!
//! ## Rendering discipline
//!
//! Card text ALWAYS rides the shaped pipeline (RM2's
//! real-font rasterisation, via
//! [`crate::media::composite_items`]) — never the 3×5
//! bitmap stamp, not even when the project's subtitle
//! style selects `SubtitleFont::Pixel`: a Pixel choice
//! is a choice about cue text, and a card burned as `?`
//! glyphs would be the P0-2 defect in a new place. The
//! plate is drawn by [`composite_title_card`] itself
//! (the media item vocabulary has no plate item — Text
//! and Image only), then the two text lines composite
//! through the funnel's own shaped path.
//!
//! Like subtitles, a card is FRAME FURNITURE: its
//! layout is computed in stage space and pinned to the
//! picture, so camera pan/zoom/rotation never move it,
//! and the layout scales with the render scale exactly
//! as the subtitle burn-in's does (IA-R10-3's rule).
//!
//! ## Styling
//!
//! There is no separate title style on the project;
//! the card derives from the project's subtitle style,
//! sanitised at read like every subtitle surface:
//! text colour for both lines, backing colour for the
//! plate, font scale for the sizes (title =
//! [`TITLE_SIZE_FACTOR`] × the subtitle size, eyebrow =
//! [`EYEBROW_SIZE_FACTOR`] ×). One styling citizen,
//! nothing new to keep in sync.
//!
//! ## Windows and fades
//!
//! A card's window is half-open, the house range shape:
//! `[chapter.start_frame, chapter.start_frame +
//! duration)`, cut at the chapter's own end — a card
//! never outlives its chapter, and a following chapter
//! starting inside the window does not cut it (episode
//! order resolves overlaps: the FIRST chapter whose
//! window holds the frame owns the card, the
//! `chapter_at` posture). The duration is stored in
//! SECONDS and converted through the project's fps at
//! read, so an fps retime (RM21) keeps the card's
//! on-screen length instead of silently stretching it.
//! The card fades in and out over
//! [`TITLE_CARD_FADE_SECONDS`] at each end (shorter on
//! a short card: a quarter of the window at most).

use serde::{Deserialize, Serialize};

use crate::model::Project;

/// The card duration a project carries when none was
/// stored, in seconds (and the fallback for a
/// non-finite stored value).
pub const TITLE_CARD_DEFAULT_SECONDS: f32 = 2.5;
/// The shortest duration a write path keeps.
pub const TITLE_CARD_MIN_SECONDS: f32 = 0.5;
/// The longest duration a write path keeps.
pub const TITLE_CARD_MAX_SECONDS: f32 = 10.0;
/// Fade length at each end of a card's window, in
/// seconds (capped at a quarter of the window).
pub const TITLE_CARD_FADE_SECONDS: f32 = 0.3;
/// Title glyph height as a multiple of the project's
/// subtitle font size.
pub const TITLE_SIZE_FACTOR: f32 = 2.2;
/// Eyebrow ("CHAPTER n") glyph height as a multiple
/// of the project's subtitle font size.
pub const EYEBROW_SIZE_FACTOR: f32 = 0.85;
/// Plate opacity at full card alpha: the plate is the
/// backing colour blended over the frame at this
/// strength, so the artwork ghosts through, dimmed —
/// a card is an overlay, not a cut to black.
pub const TITLE_CARD_PLATE_ALPHA: f32 = 0.62;
/// Plate width as a fraction of the stage width.
pub const TITLE_CARD_PLATE_WIDTH_FRAC: f32 = 0.84;

/// How chapter title cards are presented: the master
/// switch and the hold length. Stored on
/// [`Project::title_cards`]; every field serde-defaults
/// so a pre-W2-15 file loads with the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TitleCardSettings {
    /// Whether exports burn a card at each chapter's
    /// first frames. Defaults OFF: an existing project's
    /// exports must not change until the user opts in
    /// (the wave's byte-identity discipline).
    #[serde(default)]
    pub enabled: bool,
    /// How long a card holds, in seconds (converted
    /// through the project fps at read — see the module
    /// docs). Sanitised at read into
    /// [`TITLE_CARD_MIN_SECONDS`]..=[`TITLE_CARD_MAX_SECONDS`].
    #[serde(default = "default_title_card_seconds")]
    pub duration_seconds: f32,
}

fn default_title_card_seconds() -> f32 {
    TITLE_CARD_DEFAULT_SECONDS
}

impl Default for TitleCardSettings {
    fn default() -> Self {
        TitleCardSettings {
            enabled: false,
            duration_seconds: TITLE_CARD_DEFAULT_SECONDS,
        }
    }
}

impl TitleCardSettings {
    /// The settings with the duration made safe to
    /// consume: non-finite falls back to the default, a
    /// finite value clamps into the kept range — the
    /// `SubtitleStyle::sanitised` read-path rule.
    pub fn sanitised(self) -> TitleCardSettings {
        TitleCardSettings {
            duration_seconds: if self.duration_seconds.is_finite() {
                self.duration_seconds
                    .clamp(TITLE_CARD_MIN_SECONDS, TITLE_CARD_MAX_SECONDS)
            } else {
                TITLE_CARD_DEFAULT_SECONDS
            },
            ..self
        }
    }
}

/// The card's hold length in frames at `fps`: the
/// stored seconds converted at read, at least one
/// frame.
pub fn title_card_duration_frames(settings: TitleCardSettings, fps: f32) -> usize {
    let frames = (settings.sanitised().duration_seconds * fps.max(1.0)).round();
    if frames.is_finite() {
        (frames as usize).max(1)
    } else {
        1
    }
}

/// The card window of chapter `index`:
/// `[start_frame, end)` in authored frames, or `None`
/// when the index names no chapter. The end is the
/// earlier of the duration running out and the
/// chapter's own end — a card never outlives its
/// chapter; a degenerate stored chapter (end <= start,
/// hand-edited files only) yields an empty window.
pub fn title_card_window(project: &Project, index: usize) -> Option<(usize, usize)> {
    let chapter = project.chapters.get(index)?;
    let duration = title_card_duration_frames(project.title_cards, project.fps);
    let end = chapter
        .start_frame
        .saturating_add(duration)
        .min(chapter.end_frame);
    Some((chapter.start_frame, end))
}

/// The chapter whose card is live at `frame` (an
/// authored frame index): the first in episode order
/// whose card window holds the frame — the
/// `chapter_at` overlap posture. `None` when cards are
/// disabled, in a gap, or past every window.
pub fn title_card_at(project: &Project, frame: usize) -> Option<usize> {
    if !project.title_cards.enabled {
        return None;
    }
    (0..project.chapters.len()).find(|&i| {
        title_card_window(project, i)
            .is_some_and(|(start, end)| start <= frame && frame < end)
    })
}

/// The card's two lines for chapter `index`: the
/// eyebrow (the structural number, 1-based) and the
/// title VERBATIM as stored — a title is the author's
/// words (an imported harness chapter may already read
/// "Chapter Two — The Deep Forest"; the eyebrow then
/// repeats the number, which is honest structure, not
/// an error this module rewrites).
pub fn title_card_lines(project: &Project, index: usize) -> Option<(String, String)> {
    project.chapters.get(index)?;
    Some((format!("CHAPTER {}", index + 1), {
        project.chapters[index].title.clone()
    }))
}

/// The card's opacity at continuous frame `frame_f`:
/// 0 outside the live chapter's window, rising over
/// the fade at the window's head and falling over the
/// fade at its tail. The frame is rounded to the
/// nearest authored frame first — the burn-in's own
/// liveness rule (`subtitle_overlay` rounds the same
/// way), so a card's alpha is a per-authored-frame
/// quantity, stable across the funnels.
pub fn title_card_alpha(project: &Project, frame_f: f32) -> f32 {
    let frame = frame_f.round().max(0.0) as usize;
    let Some(index) = title_card_at(project, frame) else {
        return 0.0;
    };
    let Some((start, end)) = title_card_window(project, index) else {
        return 0.0;
    };
    let window = (end - start) as f32;
    if window <= 0.0 {
        return 0.0;
    }
    let fade = (TITLE_CARD_FADE_SECONDS * project.fps.max(1.0))
        .min(window / 4.0)
        .max(0.0);
    if fade <= 0.0 {
        return 1.0;
    }
    // Sampled at the frame's CENTRE: a card whose whole
    // window is one frame still shows at full strength
    // there, instead of spending its only frame at the
    // fade's zero edge.
    let pos = frame as f32 + 0.5;
    let into = (pos - start as f32) / fade;
    let outof = (end as f32 - pos) / fade;
    into.min(outof).clamp(0.0, 1.0)
}

/// The card's layout in STAGE units (multiply by the
/// render scale for output space — the burn-in's
/// IA-R10-3 rule): both line sizes and centres and the
/// plate rect `[x, y, w, h]`. Centred on the stage —
/// a card is a full-frame beat, not edge furniture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TitleCardLayout {
    /// Glyph height of the eyebrow line.
    pub eyebrow_size: f32,
    /// Glyph height of the title line.
    pub title_size: f32,
    /// The stage x both lines centre on.
    pub center_x: f32,
    /// The eyebrow line's centre y.
    pub eyebrow_y: f32,
    /// The title line's centre y.
    pub title_y: f32,
    /// The plate rect `[x, y, w, h]`.
    pub plate: [f32; 4],
}

/// The layout for `project`'s card, from the
/// sanitised subtitle style (see the module docs).
pub fn title_card_layout(project: &Project) -> TitleCardLayout {
    let style = project.subtitle_style.sanitised();
    let base = crate::subtitles::subtitle_font_size(style);
    let title_size = base * TITLE_SIZE_FACTOR;
    let eyebrow_size = base * EYEBROW_SIZE_FACTOR;
    let (stage_w, stage_h) = (project.stage[0], project.stage[1]);
    let center_x = stage_w / 2.0;
    let center_y = stage_h / 2.0;
    // The eyebrow sits above the title, the pair
    // centred as one block: the gap between centres is
    // the sum of the half-heights plus a breathing
    // space of half an eyebrow.
    let gap = title_size / 2.0 + eyebrow_size / 2.0 + eyebrow_size * 0.5;
    let eyebrow_y = center_y - gap / 2.0;
    let title_y = center_y + gap / 2.0;
    // The plate wraps the block with a title-half of
    // air on every side.
    let pad = title_size * 0.55;
    let block_top = eyebrow_y - eyebrow_size / 2.0;
    let block_bottom = title_y + title_size / 2.0;
    let plate_w = (stage_w * TITLE_CARD_PLATE_WIDTH_FRAC).min(stage_w);
    let plate = [
        center_x - plate_w / 2.0,
        block_top - pad,
        plate_w,
        (block_bottom - block_top) + pad * 2.0,
    ];
    TitleCardLayout {
        eyebrow_size,
        title_size,
        center_x,
        eyebrow_y,
        title_y,
        plate,
    }
}

/// One frame's card overlay, built directly like the
/// subtitle burn-in's (RM6's shape): the plate rect in
/// OUTPUT pixels (stage layout × `scale`) with its
/// blend alpha, and the two lines as
/// [`crate::media::MediaItem::Text`] items in output
/// space, ready for [`crate::media::composite_items`]
/// under the identity view. `None` when no card is
/// live at the frame or its alpha has faded to
/// nothing.
pub struct TitleCardOverlay {
    /// Plate rect `[x, y, w, h]` in output pixels.
    pub plate: [f32; 4],
    /// The plate colour (`0xRRGGBB` — the subtitle
    /// style's backing colour).
    pub plate_color: u32,
    /// The plate's blend strength at this frame
    /// ([`TITLE_CARD_PLATE_ALPHA`] × the fade alpha).
    pub plate_alpha: f32,
    /// The eyebrow and title items, in draw order.
    pub items: Vec<crate::media::MediaItem>,
}

/// Build the card overlay for `frame_f` at render
/// `scale` (the funnels' scale: 1.0 from the plain
/// entries, the caller's factor from the scaled ones).
pub fn title_card_overlay(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> Option<TitleCardOverlay> {
    let alpha = title_card_alpha(project, frame_f);
    if alpha <= 0.0 {
        return None;
    }
    let frame = frame_f.round().max(0.0) as usize;
    let index = title_card_at(project, frame)?;
    let (eyebrow, title) = title_card_lines(project, index)?;
    let style = project.subtitle_style.sanitised();
    let layout = title_card_layout(project);
    let item = |y: f32, size: f32, content: String| crate::media::MediaItem::Text {
        pos: [layout.center_x * scale, y * scale],
        angle_deg: 0.0,
        content,
        size: size * scale,
        color: style.text_color,
        alpha,
    };
    Some(TitleCardOverlay {
        plate: [
            layout.plate[0] * scale,
            layout.plate[1] * scale,
            layout.plate[2] * scale,
            layout.plate[3] * scale,
        ],
        plate_color: style.box_color,
        plate_alpha: TITLE_CARD_PLATE_ALPHA * alpha,
        items: vec![
            item(layout.eyebrow_y, layout.eyebrow_size, eyebrow),
            item(layout.title_y, layout.title_size, title),
        ],
    })
}

/// Composite one frame's title card into `img` — the
/// one-call funnel entry the integrator wires at both
/// raster funnels' tails (IA-W2-15-2). Draws the plate
/// as a source-over blend of the backing colour, then
/// the text through the shaped pipeline
/// ([`crate::media::composite_items`]) — ALWAYS
/// shaped, whatever the subtitle style's font selects
/// (see the module docs). No card live = no draw.
pub fn composite_title_card(
    project: &Project,
    frame_f: f32,
    scale: f32,
    img: &mut image::RgbaImage,
) {
    let Some(overlay) = title_card_overlay(project, frame_f, scale) else {
        return;
    };
    let [px, py, pw, ph] = overlay.plate;
    let a = overlay.plate_alpha.clamp(0.0, 1.0);
    if a > 0.0 && pw > 0.0 && ph > 0.0 {
        let (sr, sg, sb) = (
            ((overlay.plate_color >> 16) & 0xFF) as f32,
            ((overlay.plate_color >> 8) & 0xFF) as f32,
            (overlay.plate_color & 0xFF) as f32,
        );
        let x0 = (px.floor() as i64).max(0);
        let y0 = (py.floor() as i64).max(0);
        let x1 = ((px + pw).ceil() as i64).min(img.width() as i64);
        let y1 = ((py + ph).ceil() as i64).min(img.height() as i64);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = img.get_pixel_mut(x as u32, y as u32);
                let da = p[3] as f32 / 255.0;
                let out_a = a + da * (1.0 - a);
                if out_a <= 0.0 {
                    continue;
                }
                // Source-over in premultiplied space,
                // un-premultiplied back to the buffer's
                // straight alpha.
                p[0] = ((sr * a + p[0] as f32 * da * (1.0 - a)) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                p[1] = ((sg * a + p[1] as f32 * da * (1.0 - a)) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                p[2] = ((sb * a + p[2] as f32 * da * (1.0 - a)) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                p[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    crate::media::composite_items(project, &overlay.items, img, &|p| p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Chapter;

    /// A project with two chapters at 24 fps:
    /// "The Deep Forest" over [24, 96) and "Finale"
    /// over [96, 240). Cards off, 2.5 s default.
    fn project() -> Project {
        let mut p = Project::new_demo();
        p.fps = 24.0;
        p.chapters = vec![
            Chapter {
                title: "The Deep Forest".into(),
                start_frame: 24,
                end_frame: 96,
            },
            Chapter {
                title: "Finale".into(),
                start_frame: 96,
                end_frame: 240,
            },
        ];
        p
    }

    fn enabled() -> Project {
        let mut p = project();
        p.title_cards.enabled = true;
        p
    }

    #[test]
    fn settings_default_off_with_the_default_duration() {
        let s = TitleCardSettings::default();
        assert!(!s.enabled, "cards are opt-in");
        assert_eq!(s.duration_seconds, TITLE_CARD_DEFAULT_SECONDS);
        // Serde: a settings object naming neither field
        // loads as the default.
        let loaded: TitleCardSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, s);
    }

    #[test]
    fn sanitised_clamps_and_repairs_the_duration() {
        let mk = |d: f32| TitleCardSettings {
            enabled: true,
            duration_seconds: d,
        }
        .sanitised()
        .duration_seconds;
        assert_eq!(mk(100.0), TITLE_CARD_MAX_SECONDS);
        assert_eq!(mk(0.0), TITLE_CARD_MIN_SECONDS);
        assert_eq!(mk(f32::NAN), TITLE_CARD_DEFAULT_SECONDS);
        assert_eq!(mk(2.5), 2.5);
    }

    #[test]
    fn duration_converts_through_fps_and_never_vanishes() {
        let s = TitleCardSettings::default();
        assert_eq!(title_card_duration_frames(s, 24.0), 60);
        assert_eq!(title_card_duration_frames(s, 12.0), 30);
        let tiny = TitleCardSettings {
            enabled: true,
            duration_seconds: 0.5,
        };
        assert_eq!(title_card_duration_frames(tiny, 1.0), 1);
    }

    #[test]
    fn window_is_the_duration_cut_at_the_chapters_end() {
        let p = enabled();
        // 2.5 s at 24 fps = 60 frames: chapter 0 runs
        // 24..96, so the card fills 24..84.
        assert_eq!(title_card_window(&p, 0), Some((24, 84)));
        // Chapter 1 is long: the duration cuts it.
        assert_eq!(title_card_window(&p, 1), Some((96, 156)));
        assert_eq!(title_card_window(&p, 2), None);
        // A chapter shorter than the duration ends the
        // card with itself.
        let mut short = enabled();
        short.chapters[0].end_frame = 40;
        assert_eq!(title_card_window(&short, 0), Some((24, 40)));
    }

    #[test]
    fn at_finds_the_live_card_and_respects_the_switch() {
        let p = enabled();
        assert_eq!(title_card_at(&p, 23), None, "before the chapter");
        assert_eq!(title_card_at(&p, 24), Some(0));
        assert_eq!(title_card_at(&p, 83), Some(0));
        assert_eq!(title_card_at(&p, 84), None, "the window is half-open");
        assert_eq!(title_card_at(&p, 96), Some(1));
        let off = project();
        assert_eq!(title_card_at(&off, 24), None, "disabled = no card");
    }

    #[test]
    fn lines_carry_the_number_and_the_verbatim_title() {
        let p = project();
        assert_eq!(
            title_card_lines(&p, 0),
            Some(("CHAPTER 1".to_string(), "The Deep Forest".to_string()))
        );
        assert_eq!(
            title_card_lines(&p, 1),
            Some(("CHAPTER 2".to_string(), "Finale".to_string()))
        );
        assert_eq!(title_card_lines(&p, 5), None);
    }

    #[test]
    fn alpha_fades_in_and_out_and_is_zero_outside() {
        let p = enabled();
        assert_eq!(title_card_alpha(&p, 0.0), 0.0);
        let first = title_card_alpha(&p, 24.0);
        assert!(
            first > 0.0 && first < 0.2,
            "the first frame is the fade's first step: {first}"
        );
        let mid = title_card_alpha(&p, 50.0);
        assert_eq!(mid, 1.0, "mid-window is full strength");
        let late = title_card_alpha(&p, 83.0);
        assert!(late > 0.0 && late < 1.0, "the tail fades: {late}");
        assert_eq!(title_card_alpha(&p, 84.0), 0.0);
        // A one-frame window has no room to fade: full.
        let mut one = enabled();
        one.title_cards.duration_seconds = 0.5;
        one.fps = 1.0;
        one.chapters[0].end_frame = 25;
        assert_eq!(title_card_alpha(&one, 24.0), 1.0);
    }

    #[test]
    fn layout_centres_the_block_and_the_plate_wraps_it() {
        let p = project();
        let l = title_card_layout(&p);
        assert_eq!(l.center_x, p.stage[0] / 2.0);
        assert_eq!(l.title_size, 20.0 * TITLE_SIZE_FACTOR);
        assert_eq!(l.eyebrow_size, 20.0 * EYEBROW_SIZE_FACTOR);
        assert!(l.eyebrow_y < l.title_y, "eyebrow above the title");
        // The plate contains both line centres and the
        // stage's vertical centre, and stays on stage.
        let (x, y, w, h) = (l.plate[0], l.plate[1], l.plate[2], l.plate[3]);
        assert!(y < l.eyebrow_y && l.title_y < y + h);
        assert!((y + h / 2.0 - p.stage[1] / 2.0).abs() < 0.01);
        assert!(x >= 0.0 && x + w <= p.stage[0]);
        // The subtitle style drives the sizes: doubling
        // the font scale doubles the card's glyphs.
        let mut big = project();
        big.subtitle_style.font_scale = 2.0;
        assert_eq!(title_card_layout(&big).title_size, l.title_size * 2.0);
    }

    #[test]
    fn overlay_scales_with_the_render_and_names_both_lines() {
        let p = enabled();
        let o = title_card_overlay(&p, 50.0, 2.0).expect("a live card");
        assert_eq!(o.items.len(), 2);
        let texts: Vec<&str> = o
            .items
            .iter()
            .map(|i| match i {
                crate::media::MediaItem::Text { content, .. } => content.as_str(),
                _ => panic!("card items are text"),
            })
            .collect();
        assert_eq!(texts, ["CHAPTER 1", "The Deep Forest"]);
        // Output space: plate and sizes doubled.
        let l = title_card_layout(&p);
        assert_eq!(o.plate[2], l.plate[2] * 2.0);
        match &o.items[1] {
            crate::media::MediaItem::Text { size, alpha, .. } => {
                assert_eq!(*size, l.title_size * 2.0);
                assert_eq!(*alpha, 1.0);
            }
            _ => unreachable!(),
        }
        assert_eq!(o.plate_alpha, TITLE_CARD_PLATE_ALPHA);
        // No card outside the window; none when off.
        assert!(title_card_overlay(&p, 90.0, 1.0).is_none());
        assert!(title_card_overlay(&project(), 50.0, 1.0).is_none());
    }

    #[test]
    fn settings_round_trip_through_the_project() {
        let mut p = enabled();
        p.title_cards.duration_seconds = 4.0;
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title_cards, p.title_cards);
        // A pre-W2-15 serialisation (no title_cards key)
        // loads with the defaults, cards off.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("title_cards");
        let old: Project = serde_json::from_value(value).unwrap();
        assert_eq!(old.title_cards, TitleCardSettings::default());
    }
}
