//! Legacy-store migration prompt (Roadmap Wave 2, seat
//! W2-65 — the GUI wiring RM9's IA-RM9-2 asked for, on
//! top of F-8's per-user data directories).
//!
//! ## What this section is
//!
//! RM9 (Roadmap Wave 1, F-8) moved the app's stores out
//! of the working directory into per-user AppDirs and
//! built the migration machinery in the shell library as
//! a two-call protocol: `shell::legacy_migration_plan`
//! (pure inspection — in shell.rs's own words, "the plan
//! IS the prompt's arm") and
//! `shell::confirm_legacy_migration` (the only half that
//! writes). The library cannot prompt, so until this
//! seat the machinery was reachable only through the
//! API: no user was ever migrated, silently or
//! otherwise. This section is the prompt half — a banner
//! on Home, mounted above the recovery banner, driven
//! by the shell's arm-and-confirm discipline.
//!
//! ## The flow
//!
//! 1. At boot, `init()` builds this section's state with
//!    [`initial_state`], which captures
//!    `shell::legacy_migration_plan()` exactly once.
//!    Planning only reads, so the capture honours
//!    IA-RM9-2's "at startup, before any store write"
//!    order — and, like `home_panel::initial_state`, it
//!    performs no writes of its own. (The `State` field
//!    and the init wiring are IA-W2-65-1, the
//!    `ConfirmAction` variant is IA-W2-65-2, the module
//!    declaration is IA-W2-65-3, and the four i18n keys
//!    are IA-W2-65-4 — see the seat's findings.)
//! 2. While the captured offer holds,
//!    [`migration_banner`] renders the offer line
//!    (`migration_offer`, with the plan's file count and
//!    total bytes filled into its slots) and one button
//!    (`migration_confirm`, "Move my files").
//! 3. The first press arms
//!    `ConfirmAction::MigrateLegacyData` and re-captures
//!    the plan; the button re-renders as its danger twin
//!    under the same key — the shell's established arm
//!    rendering. The armed label stays `migration_confirm`
//!    rather than borrowing the discard wording the
//!    session-replacing arms use: nothing here is
//!    discarded, so "Move my files" is the truthful label
//!    on both presses.
//! 4. The confirming press re-plans before it confirms
//!    (see "Gate safety"), then calls
//!    `shell::confirm_legacy_migration` and reports
//!    through `migration_done` / `migration_failed`.
//!    A boot status line ([`startup_status`]) points at
//!    the banner when the recovery offer did not already
//!    claim the line — unsaved work outranks
//!    housekeeping; both banners still render in Home.
//!
//! ## Gate safety (why both presses re-plan)
//!
//! The plan captured at boot can outlive the truth: a
//! user who ignores the banner, works, and saves has put
//! new work into the data dir, which closes shell.rs's
//! `migration_gate_open` — a data dir already in use
//! must never be offered a merge. The banner may still
//! show the stale offer (renders never re-probe; the
//! plan's census is a directory walk, not a per-frame
//! cost — the same captured-verdict discipline as Home's
//! recovery offer), but no migration can run against the
//! closed gate: the arm press and the confirm press each
//! re-plan against the disk as it stands at that moment,
//! and a `None` plan stands the press down (the confirm
//! path says so, via [`GATE_CLOSED_STATUS`]). After any
//! confirm attempt the offer is re-captured, so a
//! partial migration — a hard error aborts
//! `confirm_legacy_migration` mid-copy, leaving the
//! copies made in place — re-offers exactly its
//! remainder next time, and a completed one never offers
//! again (every destination exists, so the re-plan is
//! empty and `legacy_migration_plan` returns `None`).
//!
//! ## What is deliberately not here
//!
//! - **No dismiss / "decide later" button.** Not
//!   pressing is deciding later: the offer is re-derived
//!   at every launch until it is acted on or the gate
//!   closes. A persisted "declined" flag would be a new
//!   prefs field with a new semantic — a redesign
//!   IA-RM9-2 does not price, so this seat does not
//!   smuggle one in.
//! - **No move, ever.** `confirm_legacy_migration`
//!   copies; the legacy tree stays byte-for-byte in
//!   place. That is the design's "keep both" answer, and
//!   the offer line says so ("The old folder is kept.").
//! - **No dirty-session gate.** Arming exists in this
//!   shell to protect the session from being replaced;
//!   a migration replaces nothing in the session — it
//!   copies files the session is not reading (the
//!   stores already resolve to the new dirs). So the
//!   arm here is unconditional, exactly as IA-RM9-2
//!   specifies (first press arms, second confirms),
//!   unlike Home's session-replacing presses, which arm
//!   only while the session is dirty.

use std::io;

use rune::tok::{self, type_, Theme};
use rune::{row, text, El};
use stykanimate_rs::shell::{self, LegacyMigrationPlan, MigrationReport};

use crate::chrome_view::{btn, danger_btn};
use crate::rune_sec::i18n_wire;
use crate::{ConfirmAction, State};

/// View-state for this section: the migration offer
/// captured at startup. All file state itself lives on
/// disk and in the shell library's plan/confirm pair,
/// as with the other sections.
#[derive(Clone, Debug, Default)]
pub struct MigrationPanelState {
    /// The plan the banner offers: built by
    /// [`initial_state`] at boot, refreshed by this
    /// section's arm press, and re-captured after every
    /// confirm attempt (see the module docs' gate-safety
    /// section). `None` — the `Default` — means no
    /// offer: portable mode (the stores already resolve
    /// to the legacy layout), the no-home fallback, a
    /// closed gate (the data dir is in use), no legacy
    /// files, or a migration already completed.
    pub offer: Option<LegacyMigrationPlan>,
}

/// The panel state `init()` should start from (instead
/// of `MigrationPanelState::default()`): the startup
/// probe, run once. It performs **no writes** — the
/// plan is pure inspection — so the offer is judged on
/// the state the user left behind, before any store
/// write of the new session (IA-RM9-2's order; the
/// `home_panel::initial_state` discipline).
pub fn initial_state() -> MigrationPanelState {
    MigrationPanelState {
        offer: shell::legacy_migration_plan(),
    }
}

/// The status line the shell shows at boot when a
/// migration is offered but the recovery offer did not
/// claim the line (see [`startup_status`]).
const STARTUP_MIGRATION_STATUS: &str =
    "Files from the old projects folder can be moved to the app data folder — open Home to review.";

/// The status line for the arm press: what the
/// confirming press will do, stated before it does it.
/// A literal, in the house style of the other sections'
/// arm statuses (home_panel's and project_tools' arm
/// lines are literals, not keys).
const ARM_STATUS: &str =
    "Press Move my files again to copy your files to the app data folder. The old folder is kept.";

/// The status line when the confirming press finds the
/// migration gate closed — the data dir gained the
/// user's own work since the offer was captured, so the
/// merge the gate exists to forbid is refused, plainly.
/// A literal for the same reason as [`ARM_STATUS`];
/// `migration_failed` is not reused because nothing
/// failed — the move was declined by the gate, not by
/// an I/O error.
const GATE_CLOSED_STATUS: &str =
    "Your files were not moved — the app data folder is already in use.";

/// The boot status line: `Some` while a migration can
/// be offered, `None` otherwise (boot normally, no
/// message). Read-only, like [`initial_state`] — the
/// probe is `shell::legacy_migration_plan`, which never
/// writes, so calling it from `init()` cannot migrate
/// anything.
pub fn startup_status() -> Option<String> {
    startup_status_for(shell::legacy_migration_plan().is_some())
}

fn startup_status_for(offered: bool) -> Option<String> {
    if offered {
        Some(STARTUP_MIGRATION_STATUS.into())
    } else {
        None
    }
}

// ---- Offer text (the four IA-RM9-2 keys, slot-filled) ----

/// The banner's offer line: the `migration_offer`
/// template with the plan's own numbers in its
/// `{files}` / `{bytes}` slots (the shell's
/// `tr(...).replace(...)` convention, audio_panel's
/// pattern). Kept slot-filling over plain numbers so it
/// is pinnable without a plan — a plan cannot be
/// fabricated outside shell.rs (its `_at` seams are
/// private), which is why this helper takes the counts
/// rather than the plan.
pub fn offer_line(template: &str, files: usize, bytes: u64) -> String {
    template
        .replace("{files}", &files.to_string())
        .replace("{bytes}", &bytes.to_string())
}

/// The success status: the `migration_done` template
/// with the report's copied count in its `{copied}`
/// slot.
pub fn done_line(template: &str, report: &MigrationReport) -> String {
    template.replace("{copied}", &report.copied.to_string())
}

/// The failure status: the `migration_failed` template
/// with the I/O error in its `{error}` slot.
pub fn failed_line(template: &str, error: &io::Error) -> String {
    template.replace("{error}", &error.to_string())
}

// ---- The press (two-press discipline) ----

/// What one press of the banner's button does, decided
/// purely so the table is pinnable: the handler below is
/// a thin composition of this decision with the shell's
/// plan/confirm pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationPress {
    /// First press: arm the confirm (and refresh the
    /// captured plan, per the gate-safety rule).
    Arm,
    /// Confirming press: re-plan, confirm, report.
    Run,
    /// No offer (or no longer an offer): clear any arm
    /// and do nothing else.
    StandDown,
}

/// The press table: an offer with no arm arms; an offer
/// with the arm runs; without an offer every press
/// stands down, whatever the arm says (an arm left over
/// from a spent offer must never fire a migration).
pub fn press_decision(armed: bool, offered: bool) -> MigrationPress {
    match (armed, offered) {
        (_, false) => MigrationPress::StandDown,
        (false, true) => MigrationPress::Arm,
        (true, true) => MigrationPress::Run,
    }
}

/// The banner button's press: arm, or — on the armed
/// re-render — run the migration. Any press of any
/// other control stands the arm down through the
/// shell's shared machinery (the `confirm_action`
/// field's poll/Escape discipline), exactly as with the
/// other armed actions.
fn migration_pressed(s: &mut State) {
    let armed = s.confirm_action == Some(ConfirmAction::MigrateLegacyData);
    match press_decision(armed, s.migration_panel.offer.is_some()) {
        MigrationPress::StandDown => {
            s.confirm_action = None;
        }
        MigrationPress::Arm => {
            // Refresh the captured plan before arming, so
            // the offer the user is about to confirm is
            // the gate as it stands now, not as it stood
            // at boot — and if the gate has closed, there
            // is nothing to arm.
            s.migration_panel.offer = shell::legacy_migration_plan();
            if s.migration_panel.offer.is_none() {
                s.confirm_action = None;
                return;
            }
            s.confirm_action = Some(ConfirmAction::MigrateLegacyData);
            s.core.status = ARM_STATUS.into();
        }
        MigrationPress::Run => {
            s.confirm_action = None;
            run_migration(s);
        }
    }
}

/// The confirming half: re-plan against the disk as it
/// stands at this moment (never confirm the boot plan
/// blind — see the module docs' gate-safety section),
/// then confirm and report. Whatever the outcome, the
/// captured offer is re-probed afterwards: a completed
/// migration offers nothing, a partial one offers
/// exactly its remainder, and a refused one (gate
/// closed) clears.
fn run_migration(s: &mut State) {
    let Some(plan) = shell::legacy_migration_plan() else {
        s.migration_panel.offer = None;
        s.core.status = GATE_CLOSED_STATUS.into();
        return;
    };
    match shell::confirm_legacy_migration(&plan) {
        Ok(report) => {
            s.core.status = done_line(&i18n_wire::tr(&s.core, "migration_done"), &report);
        }
        Err(e) => {
            s.core.status = failed_line(&i18n_wire::tr(&s.core, "migration_failed"), &e);
        }
    }
    s.migration_panel.offer = shell::legacy_migration_plan();
}

// ---- Elements ----

/// The migration banner: rendered only while the
/// captured offer holds (mounted by the Home panel
/// above its recovery banner — the migration offer is
/// the perishable one, and on a first-launch screen it
/// is the older claim). Returns `None` when there is
/// nothing to offer, so the mount is a single
/// conditional child and sessions without an offer
/// render exactly as before. The button is the
/// shell's arm pair under one key (`home-migrate`):
/// the plain `btn` at rest, the `danger_btn` re-render
/// while armed, both labelled `migration_confirm` (see
/// the module docs for why the armed label does not
/// change).
pub fn migration_banner(s: &State, t: &Theme) -> Option<El> {
    let plan = s.migration_panel.offer.as_ref()?;
    let armed = s.confirm_action == Some(ConfirmAction::MigrateLegacyData);
    let accent = tok::accent(0, true);
    let offer = offer_line(
        &i18n_wire::tr(&s.core, "migration_offer"),
        plan.file_count(),
        plan.total_bytes(),
    );
    let confirm_label = i18n_wire::tr(&s.core, "migration_confirm");
    Some(
        row()
            .key("home-migration")
            .w_full()
            .gap(8.0)
            .center_y()
            .px(10.0)
            .py(6.0)
            .radius(8.0)
            .fill(accent.alpha(0.18))
            .child(
                text(offer)
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0)
                    // The sentence is long prose at LABEL
                    // charges in a narrow card: it wraps,
                    // the recovery banner's treatment.
                    .wrap(),
            )
            .child(if armed {
                danger_btn("home-migrate", &confirm_label, t, migration_pressed)
            } else {
                btn("home-migrate", &confirm_label, false, t, migration_pressed)
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_line_fills_both_slots() {
        // IA-RM9-2's English template, verbatim: the
        // counts land in their slots and nothing else
        // moves.
        let line = offer_line(
            "Move {files} files ({bytes} bytes) from the old projects folder \
             to this app's data folder? The old folder is kept.",
            7,
            123_456,
        );
        assert_eq!(
            line,
            "Move 7 files (123456 bytes) from the old projects folder \
             to this app's data folder? The old folder is kept."
        );
    }

    #[test]
    fn done_line_fills_the_copied_slot() {
        let report = MigrationReport {
            copied: 5,
            skipped_existing: 2,
            bytes: 42,
        };
        assert_eq!(
            done_line("Moved {copied} files to the app data folder.", &report),
            "Moved 5 files to the app data folder."
        );
    }

    #[test]
    fn failed_line_fills_the_error_slot() {
        let err = io::Error::new(io::ErrorKind::PermissionDenied, "disk is read-only");
        assert_eq!(
            failed_line("File move failed: {error}", &err),
            "File move failed: disk is read-only"
        );
    }

    #[test]
    fn press_decision_is_the_two_press_table() {
        assert_eq!(press_decision(false, true), MigrationPress::Arm);
        assert_eq!(press_decision(true, true), MigrationPress::Run);
        // No offer: every press stands down, armed or
        // not — a stale arm must never fire a migration.
        assert_eq!(press_decision(false, false), MigrationPress::StandDown);
        assert_eq!(press_decision(true, false), MigrationPress::StandDown);
    }

    #[test]
    fn default_state_offers_nothing() {
        assert!(MigrationPanelState::default().offer.is_none());
    }

    #[test]
    fn startup_status_line_follows_the_offer() {
        assert_eq!(
            startup_status_for(true).as_deref(),
            Some(STARTUP_MIGRATION_STATUS)
        );
        assert_eq!(startup_status_for(false), None);
    }

    #[test]
    fn banner_is_none_without_an_offer() {
        // The full-State precedent (home_panel's gate
        // tests): force the resting state so the pin is
        // deterministic whatever the test machine's own
        // directories hold.
        let mut s = crate::init();
        s.migration_panel.offer = None;
        let t = crate::rune_sec::app_theme::app_theme(true);
        assert!(migration_banner(&s, &t).is_none());
    }

    #[test]
    fn press_stands_down_without_an_offer() {
        // An arm left over from a spent offer: the press
        // clears it and touches nothing else — no arm,
        // no migration, no status of its own.
        let mut s = crate::init();
        s.migration_panel.offer = None;
        s.confirm_action = Some(ConfirmAction::MigrateLegacyData);
        let status_before = s.core.status.clone();
        migration_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "the stale arm stands down");
        assert!(s.migration_panel.offer.is_none());
        assert_eq!(s.core.status, status_before, "standing down is silent");
    }
}
