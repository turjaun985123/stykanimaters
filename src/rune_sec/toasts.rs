//! Toasts for the Rune editor shell (Phase D, section 26).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! A toast is a one-line status echo (the text of
//! `EditorCore::status` after an op) shown briefly in a
//! floating stack at the window's bottom-right corner, then
//! gone. This module owns the model ([`ToastsState`]) and
//! the builder ([`toasts_view`]).
//!
//! Phase F gives the stack one action: when the core has an
//! undo step available at render time, the *newest* live
//! toast carries an Undo button (`toast-undo`) that runs
//! `core.undo()` and dismisses that toast. That is why
//! [`toasts_view`] takes the whole shell `State`, not just
//! the toast model.
//!
//! Phase G adds dismissal: every live toast carries a
//! small "×" button (`toast-dismiss-{index}`) that removes
//! exactly that toast, and when two or more toasts are live
//! the stack ends with a "Clear all" button
//! (`toast-clear-all`) that removes every live toast at
//! once ([`ToastsState::dismiss_all`]).
//!
//! Why hand-rolled instead of rune-kit's `Toast` /
//! `ToastRegion` (read at `crates/rune-kit/src/toast.rs` in the
//! pinned Rune tree):
//!
//! - Both kit types are built against a `&Kit`
//!   (`Toast::build(&Kit)`, `ToastRegion::build(&Kit)`), and
//!   this shell never constructs a `Kit` — every other section
//!   themes from `rune::tok` (see `chrome_view`). Kit carries
//!   its own OpenCode `Palette`, a second theming system for
//!   one corner stack.
//! - Kit `Toast` has no severity concept: the card fill is
//!   uniformly `c.layer_01` and the lead icon is painted
//!   `c.icon_base` whatever the message means. Severity colour
//!   is the point of this model.
//! - Kit `Toast` is a rich notification (title + description +
//!   action buttons + a close button wired to app-state
//!   handlers); these toasts are single-line status echoes
//!   whose only actions are the newest toast's Undo button,
//!   a per-toast dismiss "×", and a stack-wide "Clear all".
//! - What IS borrowed: `ToastRegion`'s proven placement —
//!   pinned 32 pt from the right, 48 pt from the bottom, 12 pt
//!   apart, 320 pt wide cards, 8 pt radius, 12 pt padding.
//!
//! Feeding contract (the integrator's side): when
//! `core.status` changes, call
//! `state.toasts.push(status_text, severity, now)`; call
//! `state.toasts.expire(now)` from the poll hook so dead
//! toasts leave the model even while nothing is pushed. The
//! view additionally filters by age, so a missed `expire`
//! never shows a stale toast. `now` / `born` are seconds on
//! the `AppSpec::poll` clock (`f64` there, cast to `f32`).
//! Mount [`toasts_view`] as a direct child of the root element
//! so the pin anchors to the window box.
//!
//! Cards are capped at 320 pt wide (the column carries the
//! same `max_w`), and the status text wraps *anywhere*: a
//! long export recap contains unbroken file paths whose
//! min-content width would otherwise stretch the card past
//! its fixed width.

use rune::tok::{self, type_, Theme};
use rune::{col, el, oklch, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::i18n_wire;
use crate::State;

/// Most toasts on screen at once; a push past the cap drops
/// the oldest toast.
pub const MAX_TOASTS: usize = 4;

/// Seconds a toast stays live. A toast expires when its age
/// is strictly greater than this (`age > TOAST_TTL`); at
/// exactly `TOAST_TTL` it is still shown.
pub const TOAST_TTL: f32 = 5.0;

/// Minimum live toasts for the stack to offer "Clear all":
/// with fewer than two live toasts, the per-toast "×" is
/// dismissal enough.
pub const CLEAR_ALL_MIN: usize = 2;

/// The last slice of a toast's life over which it fades out.
const FADE_OUT: f32 = 1.0;

/// How much a toast means it. The integrator maps status
/// text to a severity; unclassified statuses are `Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Info,
    Success,
    Warn,
    Error,
}

impl Severity {
    /// The severity's dot colour: bright OKLCH hues chosen to
    /// read on the dark card at small size. (The shell is
    /// dark-only today — `tok::theme(true)` — so these are
    /// tuned for dark.)
    pub fn color(self) -> Color {
        match self {
            Severity::Info => oklch(0.70, 0.14, 250.0),
            Severity::Success => oklch(0.72, 0.15, 155.0),
            Severity::Warn => oklch(0.78, 0.15, 75.0),
            Severity::Error => oklch(0.68, 0.20, 25.0),
        }
    }
}

/// One toast: what it says, how much it means it, and when
/// (on the poll clock, in seconds) it was born.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub text: String,
    pub severity: Severity,
    pub born: f32,
}

impl Toast {
    pub fn new(text: impl Into<String>, severity: Severity, born: f32) -> Toast {
        Toast {
            text: text.into(),
            severity,
            born,
        }
    }

    /// Seconds since birth at `now`. Negative if the clock
    /// ever runs backwards — such a toast is simply young.
    pub fn age(&self, now: f32) -> f32 {
        now - self.born
    }

    /// True once the toast is older than [`TOAST_TTL`].
    pub fn expired(&self, now: f32) -> bool {
        self.age(now) > TOAST_TTL
    }

    /// Opacity for the view: fully opaque until the last
    /// [`FADE_OUT`] seconds of life, then a linear fade to 0
    /// at the expiry instant.
    fn fade(&self, now: f32) -> f32 {
        ((TOAST_TTL - self.age(now)) / FADE_OUT).clamp(0.0, 1.0)
    }
}

/// The toast stack's model. Owns at most [`MAX_TOASTS`]
/// toasts, oldest first.
#[derive(Debug, Clone, Default)]
pub struct ToastsState {
    pub items: Vec<Toast>,
}

impl ToastsState {
    pub fn new() -> ToastsState {
        ToastsState::default()
    }

    /// Show a status line.
    ///
    /// - Stale toasts are expired first (at `now`), so they
    ///   never eat cap slots.
    /// - A push identical to the newest toast — same text AND
    ///   same severity — does not stack a copy; it refreshes
    ///   that toast's birth time, so repeating the same op
    ///   keeps one toast alive instead of piling up four.
    ///   Non-consecutive duplicates are kept.
    /// - Past [`MAX_TOASTS`], the oldest toast is dropped.
    pub fn push(&mut self, text: impl Into<String>, severity: Severity, now: f32) {
        self.expire(now);
        let text = text.into();
        if let Some(last) = self.items.last_mut() {
            if last.text == text && last.severity == severity {
                last.born = now;
                return;
            }
        }
        self.items.push(Toast::new(text, severity, now));
        while self.items.len() > MAX_TOASTS {
            self.items.remove(0);
        }
    }

    /// Drop every toast older than [`TOAST_TTL`] at `now`.
    pub fn expire(&mut self, now: f32) {
        self.items.retain(|t| !t.expired(now));
    }

    /// The toasts alive at `now`, oldest first, without
    /// mutating the state — the view renders exactly these.
    pub fn live(&self, now: f32) -> impl Iterator<Item = &Toast> {
        self.items.iter().filter(move |t| !t.expired(now))
    }

    /// How many toasts are alive at `now`.
    pub fn live_count(&self, now: f32) -> usize {
        self.live(now).count()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Remove the toast at `index` in `items` (the items
    /// index, not the live position — an expired toast that
    /// `expire` has not yet collected still occupies its
    /// slot). Out-of-range is a no-op. The newest toast's
    /// Undo action dismisses exactly its own toast this way
    /// after running the undo.
    pub fn dismiss(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
        }
    }

    /// Remove every toast alive at `now` — the "Clear all"
    /// action. Expired toasts stay stored, exactly as they
    /// do between `expire` calls: collecting them is
    /// `expire`'s job (the poll hook runs it), and `live`
    /// already hides them from the view, so after this call
    /// nothing live remains.
    pub fn dismiss_all(&mut self, now: f32) {
        self.items.retain(|t| t.expired(now));
    }

    /// The items index of the newest toast alive at `now` —
    /// the one that carries the Undo action — or `None` when
    /// nothing is live.
    pub fn newest_live_index(&self, now: f32) -> Option<usize> {
        self.items.iter().rposition(|t| !t.expired(now))
    }
}

/// True when the core has at least one undo step — the
/// render-time condition for the newest toast's Undo
/// action.
pub fn undo_available(core: &EditorCore) -> bool {
    core.undo_depth() > 0
}

/// True when a stack with `live_count` live toasts offers
/// the "Clear all" button: at least [`CLEAR_ALL_MIN`] live
/// toasts. Pure so the threshold is testable without a
/// shell `State`.
fn clear_all_offered(live_count: usize) -> bool {
    live_count >= CLEAR_ALL_MIN
}

/// The floating toast column: pinned to the bottom-right of
/// its parent (32 pt right, 48 pt bottom — the rune-kit
/// `ToastRegion` placement), newest toast nearest the
/// corner. An empty stack builds an empty pinned column,
/// which paints nothing.
///
/// When [`undo_available`] holds, the newest live toast
/// carries the Undo action (see [`toast_row`]); the
/// predicate is read at render time, so the button appears
/// and disappears with the core's undo stack.
///
/// Every live toast carries a dismiss "×"; when at least
/// [`CLEAR_ALL_MIN`] toasts are live, a "Clear all" button
/// closes the stack and removes them all at once.
pub fn toasts_view(s: &State, now: f32) -> El {
    let t = tok::theme(true);
    let mut column = col()
        .key("toasts")
        .pin(None, None, Some(32.0), Some(48.0))
        .max_w(320.0)
        .gap(12.0);
    let action_index = if undo_available(&s.core) {
        s.toasts.newest_live_index(now)
    } else {
        None
    };
    let undo_label = i18n_wire::tr(&s.core, "undo");
    for (index, toast) in s
        .toasts
        .items
        .iter()
        .enumerate()
        .filter(|(_, toast)| !toast.expired(now))
    {
        column = column.child(toast_row(
            toast,
            index,
            &t,
            now,
            action_index == Some(index),
            &undo_label,
        ));
    }
    if clear_all_offered(s.toasts.live_count(now)) {
        column = column.child(btn("toast-clear-all", "Clear all", false, &t, move |s: &mut State| {
            s.toasts.dismiss_all(now);
        }));
    }
    column
}

/// One toast card: a severity dot and the status text on the
/// theme card surface, fading over its last second. `index`
/// is the toast's items index — it keys the card (stable for
/// the toast's whole life, unlike a live position) and the
/// row's buttons dismiss by it. When `action` holds — this
/// is the newest live toast and the core can undo — the card
/// also carries the Undo button: pressing it undoes once and
/// dismisses this toast (the undo's own status change feeds
/// a fresh toast through the poll hook, as for any op).
/// Every card ends with a dismiss "×" (`toast-dismiss-{index}`)
/// that removes exactly this toast and nothing else.
fn toast_row(toast: &Toast, index: usize, t: &Theme, now: f32, action: bool, undo_label: &str) -> El {
    let mut card = row()
        .key(format!("toast-{index}"))
        .w(320.0)
        .max_w(320.0)
        .padding(12.0)
        .gap(10.0)
        .items_center()
        .radius(8.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .opacity(toast.fade(now))
        .child(
            el()
                .size(9.0, 9.0)
                .radius(4.5)
                .fill(toast.severity.color()),
        )
        .child(
            text(toast.text.clone())
                .font(type_::LABEL)
                .color(t.text)
                .wrap_anywhere()
                .grow(1.0),
        );
    if action {
        card = card.child(btn("toast-undo", undo_label, false, t, move |s: &mut State| {
            s.core.undo();
            s.toasts.dismiss(index);
        }));
    }
    let dismiss_key = format!("toast-dismiss-{index}");
    card = card.child(btn(&dismiss_key, "×", false, t, move |s: &mut State| {
        s.toasts.dismiss(index);
    }));
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(texts: &[&str], born: f32) -> ToastsState {
        let mut s = ToastsState::new();
        for text in texts {
            s.push(*text, Severity::Info, born);
        }
        s
    }

    #[test]
    fn new_state_is_empty() {
        let s = ToastsState::new();
        assert!(s.is_empty());
        assert_eq!(s.live_count(0.0), 0);
        assert!(ToastsState::default().is_empty());
    }

    #[test]
    fn push_stores_text_severity_and_birth_time() {
        let mut s = ToastsState::new();
        s.push("Saved.", Severity::Success, 2.5);
        assert_eq!(s.items.len(), 1);
        let toast = &s.items[0];
        assert_eq!(toast.text, "Saved.");
        assert_eq!(toast.severity, Severity::Success);
        assert_eq!(toast.born, 2.5);
    }

    #[test]
    fn push_dedupes_consecutive_identical_and_refreshes_birth() {
        let mut s = ToastsState::new();
        s.push("Saved.", Severity::Success, 0.0);
        s.push("Saved.", Severity::Success, 3.0);
        assert_eq!(s.items.len(), 1, "identical consecutive push must not stack");
        assert_eq!(s.items[0].born, 3.0, "the surviving toast is reborn at the latest push");
        // The refresh is real lifetime: alive 4.9 s after the
        // refresh, gone 5.1 s after it.
        assert_eq!(s.live_count(7.9), 1);
        s.expire(8.1);
        assert!(s.is_empty());
    }

    #[test]
    fn push_same_text_with_different_severity_is_not_a_duplicate() {
        let mut s = ToastsState::new();
        s.push("Export failed.", Severity::Info, 0.0);
        s.push("Export failed.", Severity::Error, 0.0);
        assert_eq!(s.items.len(), 2);
    }

    #[test]
    fn push_non_consecutive_duplicate_is_kept() {
        let mut s = ToastsState::new();
        s.push("A", Severity::Info, 0.0);
        s.push("B", Severity::Info, 0.0);
        s.push("A", Severity::Info, 0.0);
        assert_eq!(s.items.len(), 3);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["A", "B", "A"]);
    }

    #[test]
    fn push_caps_at_max_dropping_the_oldest() {
        let mut s = state_with(&["one", "two", "three", "four"], 0.0);
        s.push("five", Severity::Info, 0.0);
        assert_eq!(s.items.len(), MAX_TOASTS);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["two", "three", "four", "five"]);
    }

    #[test]
    fn push_dedupe_refresh_at_cap_drops_nothing() {
        let mut s = state_with(&["one", "two", "three", "four"], 0.0);
        s.push("four", Severity::Info, 1.0);
        assert_eq!(s.items.len(), MAX_TOASTS);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["one", "two", "three", "four"]);
        assert_eq!(s.items[3].born, 1.0);
    }

    #[test]
    fn expire_removes_only_toasts_older_than_the_ttl() {
        let mut s = ToastsState::new();
        s.items.push(Toast::new("young", Severity::Info, 5.1)); // age 4.9 at t=10
        s.items.push(Toast::new("edge", Severity::Info, 5.0)); // age exactly 5.0
        s.items.push(Toast::new("old", Severity::Info, 4.9)); // age 5.1
        s.expire(10.0);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["young", "edge"], "age == TTL is still live; only age > TTL expires");
    }

    #[test]
    fn expire_on_an_empty_state_is_a_noop() {
        let mut s = ToastsState::new();
        s.expire(100.0);
        assert!(s.is_empty());
    }

    #[test]
    fn push_expires_stale_toasts_so_they_never_eat_cap_slots() {
        let mut s = state_with(&["one", "two", "three", "four"], 0.0);
        // All four are stale by t=6; a push then must leave a
        // stack of one, not drop the new toast to cap pressure.
        s.push("fresh", Severity::Info, 6.0);
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].text, "fresh");
    }

    #[test]
    fn live_filters_expired_without_mutating() {
        let mut s = ToastsState::new();
        s.items.push(Toast::new("gone", Severity::Info, 0.0));
        s.items.push(Toast::new("here", Severity::Info, 9.0));
        let live: Vec<&str> = s.live(10.0).map(|t| t.text.as_str()).collect();
        assert_eq!(live, ["here"]);
        assert_eq!(s.live_count(10.0), 1);
        assert_eq!(s.items.len(), 2, "live() is a filter, not an expiry");
    }

    #[test]
    fn toast_age_and_expiry_boundary() {
        let toast = Toast::new("x", Severity::Warn, 2.0);
        assert_eq!(toast.age(2.0), 0.0);
        assert_eq!(toast.age(4.5), 2.5);
        assert!(!toast.expired(7.0), "age == TTL is not expired");
        assert!(toast.expired(7.01));
    }

    #[test]
    fn severity_colors_are_pairwise_distinct() {
        let colors = [
            Severity::Info.color(),
            Severity::Success.color(),
            Severity::Warn.color(),
            Severity::Error.color(),
        ];
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b, "severity colours must not collide");
            }
            assert_eq!(a.a, 1.0, "severity dots are fully opaque");
        }
    }

    #[test]
    fn fade_is_full_until_the_last_second_then_linear() {
        let toast = Toast::new("x", Severity::Info, 0.0);
        assert_eq!(toast.fade(0.0), 1.0);
        assert_eq!(toast.fade(4.0), 1.0);
        assert_eq!(toast.fade(4.5), 0.5);
        assert_eq!(toast.fade(5.0), 0.0);
    }

    #[test]
    fn view_builds_for_empty_live_and_all_expired_states() {
        let mut s = crate::init();
        let _ = toasts_view(&s, 0.0);
        s.toasts.push("one", Severity::Info, 0.0);
        s.toasts.push("two", Severity::Info, 0.0);
        let _ = toasts_view(&s, 1.0);
        // Nothing alive at t=100: the builder must still
        // produce the (empty) pinned column, not panic.
        let _ = toasts_view(&s, 100.0);
        s.toasts.clear();
        assert!(s.toasts.is_empty());
    }

    #[test]
    fn view_builds_with_the_undo_action_available() {
        // A live toast plus an undoable core exercises the
        // action branch (the button attaches to the newest
        // live row; presence in the tree is the Runtime
        // tests' job, not this smoke build's).
        let mut s = crate::init();
        s.core.add_frame();
        s.toasts.push("Frame added.", Severity::Info, 0.0);
        let _ = toasts_view(&s, 0.5);
    }

    #[test]
    fn dismiss_removes_only_the_addressed_toast() {
        let mut s = state_with(&["one", "two", "three"], 0.0);
        s.dismiss(1);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["one", "three"]);
        s.dismiss(0);
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].text, "three");
    }

    #[test]
    fn dismiss_out_of_range_is_a_noop() {
        let mut s = state_with(&["one"], 0.0);
        s.dismiss(1);
        s.dismiss(usize::MAX);
        assert_eq!(s.items.len(), 1);
        let mut empty = ToastsState::new();
        empty.dismiss(0);
        assert!(empty.is_empty());
    }

    #[test]
    fn newest_live_index_skips_an_expired_tail() {
        let mut s = ToastsState::new();
        s.items.push(Toast::new("old", Severity::Info, 0.0));
        s.items.push(Toast::new("newer", Severity::Info, 4.0));
        s.items.push(Toast::new("stale", Severity::Info, 0.0));
        // At t=6 the first and third are expired (age 6 >
        // TTL); only "newer" (age 2) is live, at items
        // index 1 — not the last slot.
        assert_eq!(s.newest_live_index(6.0), Some(1));
        assert_eq!(s.newest_live_index(100.0), None);
        assert_eq!(ToastsState::new().newest_live_index(0.0), None);
    }

    #[test]
    fn dismiss_all_removes_every_live_toast() {
        let mut s = state_with(&["one", "two", "three", "four"], 0.0);
        assert_eq!(s.live_count(1.0), 4);
        s.dismiss_all(1.0);
        assert_eq!(s.live_count(1.0), 0, "no toast stays live after Clear all");
        assert!(s.is_empty(), "all four were live, so nothing remains stored");
        assert_eq!(s.newest_live_index(1.0), None);
    }

    #[test]
    fn dismiss_all_keeps_expired_entries_stored() {
        let mut s = ToastsState::new();
        s.items.push(Toast::new("stale", Severity::Info, 0.0));
        s.items.push(Toast::new("fresh", Severity::Info, 9.0));
        // At t=10 "stale" is expired (age 10 > TTL) and
        // "fresh" is live (age 1). Clear all removes the
        // live one; the expired one stays stored, exactly
        // as it would between expire() calls.
        s.dismiss_all(10.0);
        assert_eq!(s.live_count(10.0), 0);
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].text, "stale");
        // And the poll hook's expire() still collects it.
        s.expire(10.0);
        assert!(s.is_empty());
    }

    #[test]
    fn dismiss_all_on_empty_is_a_noop() {
        let mut s = ToastsState::new();
        s.dismiss_all(0.0);
        assert!(s.is_empty());
    }

    #[test]
    fn clear_all_is_offered_only_from_two_live_toasts() {
        assert!(!clear_all_offered(0));
        assert!(!clear_all_offered(1));
        assert!(clear_all_offered(2));
        assert!(clear_all_offered(3));
        assert!(clear_all_offered(MAX_TOASTS));
    }

    #[test]
    fn view_builds_with_dismiss_buttons_and_clear_all() {
        // One live toast: the row builds with its dismiss
        // "×" and no Clear all. Three live toasts: the
        // Clear all branch builds too. (Presence in the
        // element tree is the Runtime tests' job, as with
        // the Undo smoke build above.)
        let mut s = crate::init();
        s.toasts.push("one", Severity::Info, 0.0);
        let _ = toasts_view(&s, 0.5);
        s.toasts.push("two", Severity::Info, 0.0);
        s.toasts.push("three", Severity::Warn, 0.0);
        let _ = toasts_view(&s, 0.5);
    }

    #[test]
    fn undo_available_tracks_the_core_undo_depth() {
        let mut core = EditorCore::new();
        assert!(!undo_available(&core), "a fresh core has nothing to undo");
        core.add_frame();
        assert!(undo_available(&core), "add_frame snapshots first, so undo is available");
        core.undo();
        assert!(!undo_available(&core), "the only step is spent");
    }
}
