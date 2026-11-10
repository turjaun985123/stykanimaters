//! Diagnostics section for the Rune editor shell
//! (Roadmap Wave 2, W2-96 — roadmap item E-8): the
//! Help-adjacent surface over the library's diagnostics
//! bundle ([`stykanimate_rs::diagnostics`]).
//!
//! The section is deliberately small: one explainer line
//! (the privacy contract, in the user's face before they
//! share anything), one line naming where bundles land
//! (in the bundle's own redacted role form, so the
//! section itself models the contract), and one button
//! that writes the bundle. The press is the whole op:
//! gather the live facts from the shell `State`, build,
//! write into `<data>/diagnostics`, and report through
//! `core.status` — the house channel the poll hook
//! toasts, with the full path in the status (the status
//! line is UI, not bundle content; exports report their
//! paths the same way).
//!
//! Stateless by design: the section keeps no view-state
//! of its own, so it needs no `State` field — shell
//! `State` is spelled literally in dozens of fixtures
//! and this seat adds none. The "last bundle" readout a
//! richer surface might show is exactly the kind of
//! state that would need one; the status line already
//! tells the user where the bundle went.
//!
//! Copy is English literals for now (the Help panel's
//! own strings ride the i18n catalogue through keys
//! filed by their seats): IA-W2-96-4 files the three
//! strings below as catalogue keys with the integrator's
//! splice. Nothing here renders a raw key meanwhile.
//!
//! Wiring is the integrator's: `pub mod
//! diagnostics_panel;` in `src/rune_sec/mod.rs`
//! (IA-W2-96-2) and the mount — one `.child(...)` beside
//! the Help panel's mount in `main.rs` (IA-W2-96-3).

use std::time::{SystemTime, UNIX_EPOCH};

use rune::tok::{type_, Theme};
use rune::{text, El};
use stykanimate_rs::diagnostics::{self, DiagnosticsInput, StatusFact};
use stykanimate_rs::shell;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::settings_popover::{lang_tier, LangTier};
use crate::rune_sec::toasts::Severity;
use crate::State;

/// The section title (IA-W2-96-4 files its i18n key).
const TITLE: &str = "Diagnostics";
/// What the bundle is and what it never carries, said
/// before the button is pressed.
const EXPLAINER: &str = "Writes a bug-report bundle: app, platform and project counts. \
                         It never includes project content, names, dialogue or file paths.";
/// The button's label.
const WRITE_LABEL: &str = "Write diagnostics bundle";

/// The toast severity's bundle label. The bundle speaks
/// plain lowercase severity names whatever surface
/// supplied them, so a maintainer reads one vocabulary.
fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Success => "success",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

/// The language tier's bundle label, from the same
/// [`lang_tier`] table the Settings picker renders.
fn tier_label(tier: LangTier) -> &'static str {
    match tier {
        LangTier::Full => "full",
        LangTier::Partial => "partial",
        LangTier::Basic => "basic",
    }
}

/// Assemble the bundle inputs from the live shell state.
/// Everything the state cannot honestly supply stays
/// `None` (see [`DiagnosticsInput`]): the UI scale in
/// particular — the shell never reads the host scale
/// (W2-56), so the bundle records "not recorded" rather
/// than inventing 1.0, and the home needle stays unset
/// because the shell has no home API that is not an
/// environment read (the store-role redaction and the
/// status scrubber carry the contract without it).
fn gather_input(s: &State) -> DiagnosticsInput {
    DiagnosticsInput {
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        ui_scale: None,
        language: Some(format!("{:?}", s.core.lang)),
        language_tier: Some(tier_label(lang_tier(s.core.lang)).to_string()),
        mcp_tool_count: Some(stykanimate_rs::mcp::all_tools().len()),
        undo_depth: s.core.undo_depth(),
        redo_depth: s.core.redo_depth(),
        recent_status: s
            .toasts
            .items
            .iter()
            .map(|toast| StatusFact {
                severity: severity_label(toast.severity).to_string(),
                text: toast.text.clone(),
            })
            .collect(),
        home: None,
    }
}

/// The button's press: build the bundle from the live
/// project and write it under `<data>/diagnostics`,
/// reporting the outcome through `core.status` in the
/// ops' voice (`… failed: …` classifies as an error
/// toast; the success line names the full path, as
/// export statuses do).
fn write_pressed(s: &mut State) {
    let dirs = shell::store_dirs();
    let input = gather_input(s);
    let bundle = diagnostics::build_bundle(&s.core.project, &dirs, &input);
    let out_dir = diagnostics::diagnostics_dir(&dirs);
    match diagnostics::write_bundle(&out_dir, &bundle) {
        Ok(path) => {
            s.core.status =
                format!("Diagnostics bundle written to {}", path.display());
        }
        Err(e) => {
            s.core.status = format!("Diagnostics bundle failed: {e}");
        }
    }
}

/// The section builder, in the Help panel's idiom (a
/// card column from the design kit). Pure over the
/// state: it reads the core language for nothing but
/// show — the copy is literal until IA-W2-96-4 lands —
/// and resolves the store dirs only to *name* the
/// destination in redacted role form.
pub fn diagnostics_section(s: &State, t: &Theme) -> El {
    let dirs = shell::store_dirs();
    let destination = diagnostics::redact_path(
        &diagnostics::diagnostics_dir(&dirs),
        &dirs,
        None,
    );
    let status_line = match s.toasts.items.len() {
        0 => "No status lines recorded yet this session.".to_string(),
        1 => "Includes the project census and the 1 status line from this session."
            .to_string(),
        n => format!(
            "Includes the project census and the {n} status lines from this session."
        ),
    };
    design::panel_col("help-diagnostics", t)
        .child(text(TITLE).font(type_::LABEL).color(t.text))
        .child(
            text(EXPLAINER)
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
        .child(
            text(status_line)
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
        .child(
            text(format!("Bundles are written to {destination}."))
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
        .child(btn(
            "diagnostics-write-bundle",
            WRITE_LABEL,
            false,
            t,
            write_pressed,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_labels_are_the_bundle_vocabulary() {
        assert_eq!(severity_label(Severity::Info), "info");
        assert_eq!(severity_label(Severity::Success), "success");
        assert_eq!(severity_label(Severity::Warn), "warn");
        assert_eq!(severity_label(Severity::Error), "error");
    }

    #[test]
    fn tier_labels_match_the_settings_vocabulary() {
        assert_eq!(tier_label(LangTier::Full), "full");
        assert_eq!(tier_label(LangTier::Partial), "partial");
        assert_eq!(tier_label(LangTier::Basic), "basic");
    }
}
