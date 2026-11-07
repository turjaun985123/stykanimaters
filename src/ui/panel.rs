//! Reusable shell for the editor's toggleable panels.
//!
//! The graph, audio, FX, clips, colour, easing and export-centre
//! panels in `crate::app` each hand-build the same container: a
//! full-width dark panel with a thin border, a header row holding a
//! small uppercase caption plus an optional muted subtitle, and the
//! panel body underneath. This component captures that exact shell
//! so a caller supplies only a [`PanelSpec`] and a body `Div`.
//!
//! Presentation only, like the rest of this folder: the builders
//! attach no click listeners and hold no editor state. Callers keep
//! all behaviour — they build the body (buttons included) and pass
//! it in, and they decide when the panel exists at all.

use gpui::*;

use crate::ui_kit::{section_label, top_bar, C_BORDER, C_MUTED, C_PANEL};

/// Title shown when a spec's title is empty or all whitespace.
pub const DEFAULT_PANEL_TITLE: &str = "Panel";

/// Plain display data for one panel render: its title and an
/// optional subtitle shown next to the title in the header row.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelSpec {
    /// Panel title, rendered as the header's uppercase caption.
    pub title: String,
    /// Optional descriptive text beside the title (for example a
    /// live summary such as "3 clip(s)"). Blank subtitles are
    /// treated as absent.
    pub subtitle: Option<String>,
}

impl Default for PanelSpec {
    fn default() -> Self {
        PanelSpec {
            title: DEFAULT_PANEL_TITLE.to_string(),
            subtitle: None,
        }
    }
}

impl PanelSpec {
    /// A spec with the given title and no subtitle.
    pub fn new(title: impl Into<String>) -> Self {
        PanelSpec {
            title: title.into(),
            subtitle: None,
        }
    }

    /// Set the subtitle, replacing any previous one.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The title as the header shows it: trimmed, with
    /// [`DEFAULT_PANEL_TITLE`] standing in for a blank title.
    pub fn effective_title(&self) -> &str {
        let title = self.title.trim();
        if title.is_empty() {
            DEFAULT_PANEL_TITLE
        } else {
            title
        }
    }

    /// The subtitle as the header shows it: trimmed, and `None`
    /// when the spec has no subtitle or a blank one.
    pub fn effective_subtitle(&self) -> Option<&str> {
        self.subtitle
            .as_deref()
            .map(str::trim)
            .filter(|subtitle| !subtitle.is_empty())
    }
}

/// The header row of a panel: the title as a [`section_label`],
/// followed by the subtitle in 12px muted text when the spec has
/// one. Callers may append their own children (controls, spacers)
/// to the returned bar, as the hand-built panels do.
pub fn panel_header(spec: &PanelSpec) -> Div {
    let mut bar = top_bar().child(section_label(spec.effective_title()));
    if let Some(subtitle) = spec.effective_subtitle() {
        bar = bar.child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child(subtitle.to_string()),
        );
    }
    bar
}

/// A complete panel: the shared container (panel background, thin
/// border, rounded corners, padding) with [`panel_header`] on top
/// and `body` below it. This matches, metric for metric, the
/// containers the toggleable panels in `crate::app` build inline.
pub fn panel_shell(spec: &PanelSpec, body: Div) -> Div {
    div()
        .flex_col()
        .w_full()
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .p(px(8.0))
        .gap(px(6.0))
        .child(panel_header(spec))
        .child(body)
}

#[cfg(test)]
mod tests {
    // NOTE: no `use super::*;` here — this file's `use gpui::*;`
    // would re-export GPUI's own `test` macro through the glob and
    // shadow the built-in `#[test]`, which then expands into itself
    // forever (CI failure, 2026-11-08). Import exact names only.
    use super::{PanelSpec, DEFAULT_PANEL_TITLE};

    #[test]
    fn default_spec_has_default_title_and_no_subtitle() {
        let spec = PanelSpec::default();
        assert_eq!(spec.title, DEFAULT_PANEL_TITLE);
        assert_eq!(spec.subtitle, None);
        assert_eq!(spec.effective_title(), DEFAULT_PANEL_TITLE);
        assert_eq!(spec.effective_subtitle(), None);
    }

    #[test]
    fn new_spec_takes_the_title_and_no_subtitle() {
        let spec = PanelSpec::new("Graph");
        assert_eq!(spec.title, "Graph");
        assert_eq!(spec.effective_title(), "Graph");
        assert_eq!(spec.effective_subtitle(), None);
    }

    #[test]
    fn blank_titles_fall_back_to_the_default() {
        assert_eq!(PanelSpec::new("").effective_title(), DEFAULT_PANEL_TITLE);
        assert_eq!(
            PanelSpec::new("   ").effective_title(),
            DEFAULT_PANEL_TITLE
        );
    }

    #[test]
    fn titles_are_trimmed_for_display() {
        assert_eq!(PanelSpec::new("  Graph  ").effective_title(), "Graph");
    }

    #[test]
    fn with_subtitle_sets_and_trims_the_subtitle() {
        let spec = PanelSpec::new("Export").with_subtitle("  every format  ");
        assert_eq!(spec.effective_subtitle(), Some("every format"));
    }

    #[test]
    fn blank_subtitles_are_treated_as_absent() {
        let spec = PanelSpec::new("Export").with_subtitle("   ");
        assert_eq!(spec.effective_subtitle(), None);
    }
}
