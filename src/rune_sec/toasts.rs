//! Toasts for the Rune editor shell (Phase D, section 26).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! A toast is a one-line status echo (the text of
//! `EditorCore::status` after an op) shown briefly in a
//! floating stack at the window's bottom-right corner, then
//! gone. This module owns the model ([`ToastsState`]) and a
//! pure builder ([`toasts_view`]); it has no handlers and no
//! engine knowledge.
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
//!   with no actions and no dismissal but time.
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

use rune::tok::{self, type_, Theme};
use rune::{col, el, oklch, row, text, Color, El};

/// Most toasts on screen at once; a push past the cap drops
/// the oldest toast.
pub const MAX_TOASTS: usize = 4;

/// Seconds a toast stays live. A toast expires when its age
/// is strictly greater than this (`age > TOAST_TTL`); at
/// exactly `TOAST_TTL` it is still shown.
pub const TOAST_TTL: f32 = 5.0;

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
}

/// The floating toast column: pinned to the bottom-right of
/// its parent (32 pt right, 48 pt bottom — the rune-kit
/// `ToastRegion` placement), newest toast nearest the
/// corner. An empty stack builds an empty pinned column,
/// which paints nothing.
pub fn toasts_view(state: &ToastsState, now: f32) -> El {
    let t = tok::theme(true);
    let mut column = col()
        .key("toasts")
        .pin(None, None, Some(32.0), Some(48.0))
        .gap(12.0);
    for (i, toast) in state.live(now).enumerate() {
        column = column.child(toast_row(toast, i, &t, now));
    }
    column
}

/// One toast card: a severity dot and the status text on the
/// theme card surface, fading over its last second.
fn toast_row(toast: &Toast, index: usize, t: &Theme, now: f32) -> El {
    row()
        .key(format!("toast-{index}"))
        .w(320.0)
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
                .wrap()
                .grow(1.0),
        )
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
        let empty = ToastsState::new();
        let _ = toasts_view(&empty, 0.0);
        let mut s = state_with(&["one", "two"], 0.0);
        let _ = toasts_view(&s, 1.0);
        // Nothing alive at t=100: the builder must still
        // produce the (empty) pinned column, not panic.
        let _ = toasts_view(&s, 100.0);
        s.clear();
        assert!(s.is_empty());
    }
}
