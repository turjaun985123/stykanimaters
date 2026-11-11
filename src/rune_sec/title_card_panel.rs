//! The Chapter title cards section for the Rune editor
//! shell (Roadmap Wave 2, W2-15) — the panel half of
//! P-15's title cards, mounted at the foot of the Audio
//! workspace's panel under the Subtitles section (the
//! mount is IA-W2-15-3; the section call there is the
//! whole mount, the subtitle section's precedent).
//!
//! What the section carries: the master switch and the
//! hold duration for the project's title cards (the two
//! [`stykanimate_rs::title_cards`] settings ops — one
//! undo step per real change, no-change calls spending
//! nothing), the chapter list in episode order, and a
//! live preview of the selected chapter's card.
//!
//! ## The preview is the card
//!
//! The preview renders the SAME content the export
//! burn-in draws, from the same source: the two lines
//! come from `title_cards::title_card_lines`, the
//! colours from the project's subtitle style (the
//! card's plate is its backing colour, its text the
//! style's text colour), and the size ratio follows
//! the core layout's factors. Rune text is real shaped
//! text, as is the burn-in's (RM2) — no surface draws
//! a card with the bitmap stamp. Proportions in the
//! panel are the panel's (a fixed-height plate), not
//! the stage's; the geometry itself is the core's.
//!
//! ## Selection
//!
//! Chapters have no shell selection field (W2-12's
//! settlement — none is invented). The section keeps
//! its own session selection in a `thread_local` (the
//! rig tree's pattern): view state only, guarded at
//! every read by the live chapter count, so a selection
//! left over from a longer chapter list behaves as no
//! selection. With no live selection the preview
//! follows the chapter at the playhead, then the first
//! chapter. A row press selects AND seeks the playhead
//! to the chapter's first frame (`EditorCore::goto` —
//! the dialogue lane's `press_chapter` move), spending
//! no undo step.
//!
//! Copy is literal English in this build, the rig
//! tree / graph panel precedent; the i18n string table
//! is filed as IA-W2-15-5.

use std::cell::Cell;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::title_cards;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::State;

thread_local! {
    /// The section's selected chapter (episode-order
    /// index), session state only — see the module
    /// docs. Guarded by the live chapter count at every
    /// read ([`selected_chapter`]).
    static SELECTED_CHAPTER: Cell<Option<usize>> = const { Cell::new(None) };
}

/// The live selection: the stored index while it still
/// names a chapter, else `None`.
fn selected_chapter(chapter_count: usize) -> Option<usize> {
    SELECTED_CHAPTER
        .with(|c| c.get())
        .filter(|&i| i < chapter_count)
}

/// Which chapter the preview shows: the live
/// selection, else the chapter at the playhead, else
/// the first chapter. `None` only when the project has
/// no chapters.
fn preview_chapter(s: &State) -> Option<usize> {
    let chapters = s.core.chapters();
    if chapters.is_empty() {
        return None;
    }
    selected_chapter(chapters.len())
        .or_else(|| s.core.chapter_index_at(s.core.current))
        .or(Some(0))
}

/// One chapter row's label: 1-based number, verbatim
/// title, 1-based frame span (the chapter ops' own
/// reporting shape).
fn chapter_row_label(index: usize, chapter: &stykanimate_rs::model::Chapter) -> String {
    format!(
        "{}. {} — frames {}–{}",
        index + 1,
        chapter.title,
        chapter.start_frame + 1,
        chapter.end_frame
    )
}

/// A subtitle-style colour (`0xRRGGBB`) as a Rune
/// colour, for the preview plate and text.
fn style_color(hex: u32, fallback: rune::Color) -> rune::Color {
    rune::color::hex(&format!("#{hex:06X}")).unwrap_or(fallback)
}

// ---------------------------------------------------------------------
// Handlers (each takes the shell state, the panels'
// convention; undo accounting is the core ops').
// ---------------------------------------------------------------------

/// Row press: select the chapter for the preview and
/// seek the playhead to its first frame. No undo step
/// (selection and playhead are runtime, the timeline
/// press discipline).
fn select_chapter(s: &mut State, index: usize) {
    let Some(chapter) = s.core.chapters().get(index) else {
        return;
    };
    let start = chapter.start_frame;
    SELECTED_CHAPTER.with(|c| c.set(Some(index)));
    s.core.goto(start);
}

/// The master switch: flip the stored setting through
/// its op (one undo step on the change).
fn toggle_cards(s: &mut State) {
    let on = s.core.project.title_cards.enabled;
    s.core.title_cards_set_enabled(!on);
}

/// A duration step: ±0.5 s through the op, which
/// clamps and refuses a no-change without spending.
fn step_duration(s: &mut State, delta: f32) {
    let now = s.core.project.title_cards.sanitised().duration_seconds;
    s.core.title_cards_set_duration(now + delta);
}

// ---------------------------------------------------------------------
// The section
// ---------------------------------------------------------------------

/// The card preview plate for chapter `index`: the
/// backing-colour plate with the eyebrow and title
/// centred, drawn with the project's own colours.
fn preview_block(s: &State, t: &Theme, index: usize) -> El {
    let core = &s.core;
    let (eyebrow, title) = title_cards::title_card_lines(&core.project, index)
        .unwrap_or_default();
    let style = core.project.subtitle_style.sanitised();
    let plate = style_color(style.box_color, rune::Color::BLACK);
    let ink = style_color(style.text_color, rune::Color::WHITE);
    let chapter = &core.project.chapters[index];
    let settings = core.project.title_cards.sanitised();
    let caption = if settings.enabled {
        format!(
            "Shows for {:.1} s from frame {}",
            settings.duration_seconds,
            chapter.start_frame + 1
        )
    } else {
        "Cards are off — exports will not burn this card.".to_string()
    };
    col()
        .w_full()
        .gap(6.0)
        .child(
            el()
                .key("title-card-preview")
                .w_full()
                .min_h(104.0)
                .radius(8.0)
                .fill(plate)
                .center()
                .child(
                    col()
                        .center()
                        .gap(5.0)
                        .child(
                            text(eyebrow)
                                .font(rune::tok::TextStyle::new(10.5, 600.0))
                                .color(ink),
                        )
                        .child(
                            text(title)
                                .font(rune::tok::TextStyle::new(24.0, 600.0))
                                .color(ink)
                                .truncate(),
                        ),
                ),
        )
        .child(
            text(caption)
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
}

/// The Chapter title cards section: switch, duration,
/// chapter list, preview. Renders in full for a
/// chapter-less project too (the empty state names
/// where chapters come from), the subtitle section's
/// posture.
pub fn title_card_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let chapters = core.chapters();
    let settings = core.project.title_cards;
    let mut section = col()
        .key("title-card-section")
        .w_full()
        .gap(8.0)
        .child(design::section_label("Chapter title cards", t));
    if chapters.is_empty() {
        return section.child(design::empty_state(
            "star",
            "No chapters yet — cards appear here once the episode has chapters.",
            t,
        ));
    }
    section = section.child(design::toggle_row(
        "title-cards-enabled",
        "Title cards in exports",
        settings.enabled,
        t,
        |s| toggle_cards(s),
    ));
    section = section.child(
        row()
            .gap(6.0)
            .center_y()
            .child(
                text("Card duration")
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(btn("title-card-dur-minus", "−", false, t, |s| {
                step_duration(s, -0.5)
            }))
            .child(design::stepper_value(
                &format!("{:.1} s", settings.sanitised().duration_seconds),
                t,
            ))
            .child(btn("title-card-dur-plus", "+", false, t, |s| {
                step_duration(s, 0.5)
            })),
    );
    let selected = preview_chapter(s);
    for (i, chapter) in chapters.iter().enumerate() {
        section = section.child(
            design::list_row(&format!("title-card-row-{i}"), selected == Some(i), t)
                .w_full()
                .on_press(move |s: &mut State| select_chapter(s, i))
                .child(
                    text(chapter_row_label(i, chapter))
                        .font(type_::BODY)
                        .color(t.text)
                        .grow(1.0)
                        .truncate(),
                ),
        );
    }
    if let Some(index) = selected {
        section = section.child(preview_block(s, t, index));
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::editor::EditorCore;

    /// The panels' demo-state literal (the
    /// particles_panel pattern), with chapters on the
    /// core when asked.
    fn demo_state(chapters: &[(&str, usize, usize)]) -> State {
        let mut core = EditorCore::new();
        for (title, start, end) in chapters {
            core.chapter_add(*start, *end, title);
        }
        State {
            core,
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            migration_panel: crate::rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            guides: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: Default::default(),
            segment_numbers: Default::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: None,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            guides_golden: false,
            guides_diagonals: false,
            guides_stored: true,
            rulers: false,
            limit_arcs: false,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    const TWO: &[(&str, usize, usize)] = &[
        ("The Deep Forest", 24, 96),
        ("Finale", 96, 240),
    ];

    #[test]
    fn section_constructs_on_every_branch() {
        SELECTED_CHAPTER.with(|c| c.set(None));
        for chapters in [&[][..], TWO] {
            let s = demo_state(chapters);
            for dark in [true, false] {
                let t = rune::tok::theme(dark);
                let _ = title_card_section(&s, &t);
            }
        }
        // Cards on, and a stale stored selection over a
        // shorter list: both construct, and the stale
        // selection reads as none.
        let mut s = demo_state(TWO);
        s.core.title_cards_set_enabled(true);
        SELECTED_CHAPTER.with(|c| c.set(Some(9)));
        let t = rune::tok::theme(true);
        let _ = title_card_section(&s, &t);
        assert_eq!(preview_chapter(&s), Some(0), "stale -> playhead/first");
        SELECTED_CHAPTER.with(|c| c.set(None));
    }

    #[test]
    fn row_label_is_numbered_title_and_one_based_span() {
        let chapter = stykanimate_rs::model::Chapter {
            title: "The Deep Forest".into(),
            start_frame: 24,
            end_frame: 96,
        };
        assert_eq!(
            chapter_row_label(0, &chapter),
            "1. The Deep Forest — frames 25–96"
        );
    }

    #[test]
    fn handlers_drive_the_ops_and_the_playhead() {
        SELECTED_CHAPTER.with(|c| c.set(None));
        let mut s = demo_state(TWO);
        // Toggle: one undo step, setting flipped.
        let depth = s.core.undo_depth();
        toggle_cards(&mut s);
        assert!(s.core.project.title_cards.enabled);
        assert_eq!(s.core.undo_depth(), depth + 1);
        // Duration step: +0.5 s through the op.
        step_duration(&mut s, 0.5);
        assert_eq!(s.core.project.title_cards.duration_seconds, 3.0);
        // Row press: selection set, playhead at the
        // chapter's first frame, no undo spent.
        let depth = s.core.undo_depth();
        select_chapter(&mut s, 1);
        assert_eq!(selected_chapter(2), Some(1));
        assert_eq!(s.core.current, 96);
        assert_eq!(s.core.undo_depth(), depth, "a press spends no undo");
        assert_eq!(preview_chapter(&s), Some(1));
        SELECTED_CHAPTER.with(|c| c.set(None));
    }
}
