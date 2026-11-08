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
//! small dismiss button (`toast-dismiss-{index}`) that removes
//! exactly that toast, and when two or more toasts are live
//! the stack ends with a "Clear all" button
//! (`toast-clear-all`) that removes every live toast at
//! once ([`ToastsState::dismiss_all`]).
//!
//! Phase H puts Font Awesome glyphs on the stack (via the
//! icon_btn / icons sections): the dismiss button is an
//! icon-only "xmark" square, Undo is "arrow-rotate-left"
//! plus its label, "Clear all" is "trash-can" plus its
//! label, and each card leads with its severity's glyph
//! ([`Severity::icon`]) in place of the plain dot — the
//! severity colour now tints the glyph itself.
//!
//! Phase N gives the stack its other three voices and its
//! card anatomy: severity is no longer the integrator's
//! guess — [`classify_status`] reads the status text's
//! shape (the engine's `" failed"` failures, the `"No …"` /
//! `"Nothing …"` refusals, the `"Exported …"` / `"Saved …"`
//! confirmations) and the push site passes its verdict, so
//! failures arrive red and refusals amber instead of every
//! status wearing the blue info tint. Each card now leads
//! with a 3 pt severity bar stretched to the card's full
//! height (radius 12 card, gap 8 stack), and rows enter
//! with a 24 pt slide + fade on the SETTLE spring pair
//! (p15 M3) over the existing last-second fade-out.
//!
//! Wave 6 (audit S18-11a) extends the classifier: the
//! subject-first refusal grammar (`"cannot"`, `"already"`,
//! `"at least"`, `" is empty"`) now also reads Warn, and
//! the normalise sweep's benign `"No … needed …"` outcome
//! is carved out of Warn into Info.
//!
//! Wave 9 hardens the stack against repetition and its own
//! height: [`ToastsState::push`] dedupes by (text, severity)
//! across the whole live window — before, only a duplicate
//! of the *newest* toast refreshed, so alternating ops that
//! repeat a status (pose, nudge, pose…) stacked twins — and
//! [`MAX_TOASTS`] is re-derived from the stage dock's
//! geometry: three is the largest stack that cannot reach
//! the camera pill at the smallest supported window, even
//! when every card wraps to three lines (the constant's own
//! docs carry the arithmetic).
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
//!   a per-toast dismiss button, and a stack-wide "Clear all".
//! - What IS borrowed: `ToastRegion`'s proven placement —
//!   pinned 32 pt from the right, 48 pt from the bottom,
//!   320 pt wide cards, 12 pt padding (Phase N: 8 pt stack
//!   gap, 12 pt card radius).
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
use rune::{col, el, row, text, Color, El, Transition};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::app_theme;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el_rgb;
use crate::State;

/// Most toasts on screen at once; a push past the cap drops
/// the oldest toast.
///
/// Wave 9 re-derived the number from the stage dock's
/// geometry — the old value dated from the window-corner
/// era, when the stack sat in dead space and covered
/// nothing. The stack now hangs from the stage's top-right
/// corner (12 pt insets, `main.rs`'s stage-wrap mount) and
/// grows DOWN toward the camera pill pinned at the stage's
/// bottom-right (the pill's top edge sits 41 pt above the
/// stage's bottom edge — headless tree dumps, 2026-11-08).
/// At the smallest supported window (1280×720) the stage is
/// 350 pt tall, leaving 350 − 12 − 41 = 297 pt of stack
/// room. A card measures 50 pt at one line and 54 pt at two
/// (same dumps), ≈ 15 pt per wrapped line, so a three-line
/// card is ≈ 69 pt; the stack gap is 8 pt and the Clear-all
/// row adds its 25 pt plus a gap. Three toasts stay clear
/// in every case — worst case 3 × 69 + 2 × 8 + 33 = 256 pt.
/// Four do not: four three-line cards reach 4 × 69 + 3 × 8
/// + 33 = 333 pt and would cover the pill's top edge; four
/// only fits while every card happens to stay at two lines
/// or fewer, which the pill cannot rely on.
pub const MAX_TOASTS: usize = 3;

/// Seconds a toast stays live. A toast expires when its age
/// is strictly greater than this (`age > TOAST_TTL`); at
/// exactly `TOAST_TTL` it is still shown.
pub const TOAST_TTL: f32 = 5.0;

/// Minimum live toasts for the stack to offer "Clear all":
/// with fewer than two live toasts, the per-toast dismiss
/// button is dismissal enough.
pub const CLEAR_ALL_MIN: usize = 2;

/// The last slice of a toast's life over which it fades out.
const FADE_OUT: f32 = 1.0;

/// How much a toast means it. Status text is mapped to a
/// severity by [`classify_status`]; anything it does not
/// recognise is `Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Info,
    Success,
    Warn,
    Error,
}

/// Pack a theme colour into the `0xRRGGBB` form the
/// icon renderer takes (the `fig3_preview` precedent:
/// via the colour's `#RRGGBBAA` hex string).
fn pack(c: Color) -> u32 {
    let hex = c.hex();
    u32::from_str_radix(&hex[1..7], 16).unwrap_or(0)
}

impl Severity {
    /// The severity's colour on a card surface, from the
    /// shell's one semantic palette ([`app_theme`]): Info
    /// is the theme accent; Success / Warn / Error are the
    /// brief's semantic tints in their `*_on` form — the
    /// brief value itself in dark, darkened to ≈ 5:1 on
    /// the white card in light. The consumers are the
    /// toast card's 3 pt leading bar and the export
    /// recap's text: in dark the `*_on` values ARE the
    /// base [`app_theme::success`] / [`app_theme::warning`]
    /// / [`app_theme::error_ink`] values, and in light the
    /// bar and the text darken together rather than
    /// splitting one severity into two tints.
    /// [`Severity::icon_color`] is this colour's hex twin
    /// for the glyph renderer.
    pub fn color(self, t: &Theme) -> Color {
        match self {
            Severity::Info => app_theme::accent(t),
            Severity::Success => app_theme::success_on(t),
            Severity::Warn => app_theme::warning_on(t),
            Severity::Error => app_theme::error_on(t),
        }
    }

    /// The severity's Font Awesome glyph (icons section),
    /// drawn at the card's leading edge: information, done,
    /// warning, failed. Warn and Error deliberately do not
    /// share a glyph — the triangle reads "careful", the
    /// cross reads "this failed" (and pairs with Success's
    /// tick); the glyph sits at the card's leading edge, far
    /// from the trailing dismiss square, so Error's cross is
    /// never the dismiss control.
    pub fn icon(self) -> &'static str {
        match self {
            Severity::Info => "circle-info",
            Severity::Success => "check",
            Severity::Warn => "triangle-exclamation",
            Severity::Error => "xmark",
        }
    }

    /// The glyph's hex colour, in the `0xRRGGBB` form the
    /// icon renderer takes: [`Severity::color`] packed, so
    /// the glyph carries exactly the severity tint the bar
    /// and the recap text carry, in either theme.
    pub fn icon_color(self, t: &Theme) -> u32 {
        pack(self.color(t))
    }
}

/// Classify a status line by its shape alone — the Phase N
/// replacement for the push site's old hardcoded
/// `Severity::Info`. The patterns are the engine's own
/// status grammar, sampled from the tree: this function
/// reads strings, it never rewrites them, and the wording
/// of every status stays exactly as its op wrote it.
///
/// Rules, in order (order matters — a refusal-shaped
/// failure is a failure):
///
/// 1. **Error** — contains `" failed"`: the engine's
///    failure shape (`"Audio fade failed: no clip 1."`,
///    `"Audio compress failed: output would overwrite
///    the source."`, the export failures). First, so a
///    failure whose reason wears refusal words stays a
///    failure (`"Audio crossfade failed: a clip is
///    empty."`).
/// 2. **Info** — the no-op sweep shape: starts with
///    `"No "` and contains `" needed "`. The normalise
///    op ran over every clip and found none needing the
///    change (`"No audio clips needed normalising to
///    peak 0.90."`) — a benign outcome, not a refusal,
///    carved out of rule 3 (audit S18's one false Warn;
///    the `" needed "` census is this one family).
/// 3. **Warn** — starts with `"No "` or `"Nothing "`: a
///    refusal — the op did not happen, nothing is broken
///    (`"No FX clip selected to split."`, `"No camera key
///    on this frame."`, `"Nothing copied yet."`).
/// 4. **Success** — starts with `"Exported "` or
///    `"Saved "`: a confirmation that something left the
///    editor (`"Exported 5 PNG frames to ./export"`,
///    `"Exported out/animation.apng."`). Note the
///    trailing space: the bare `"Saved."` is a neutral
///    fact, not this shape. This rule sits ahead of the
///    refusal grammar below so a confirmation can never
///    be flipped by a stray grammar word inside it.
/// 5. **Warn** — the refusal grammar (Wave 6, audit
///    S18-11a): contains `"cannot"`, `"already"`,
///    `"at least"` or `" is empty"`. These are the
///    subject-first refusals rule 3's prefixes miss —
///    the op did not happen because the state forbids
///    or already satisfies it (`"Frame 2 cannot be
///    swapped with itself."`, `"Clip 2 is already
///    muted."`, `"A movieclip needs at least one
///    frame."`, `"Keep at least one figure."`, `"3D
///    library is empty — save a 3D figure first."`,
///    `"First frame: cannot shift it earlier."`). The
///    `"at least"` arm covers the audit's `"needs at
///    least"` and `"Keep at least"` shapes at once;
///    every `"at least"` status in the tree is a
///    refusal (grep census, 2026-11-08).
/// 6. **Info** — everything else: neutral facts
///    (`"Undone."`, `"Redone."`), and the empty string,
///    which the push site never sends but which must
///    still classify without panicking.
pub fn classify_status(status: &str) -> Severity {
    if status.contains(" failed") {
        Severity::Error
    } else if status.starts_with("No ") && status.contains(" needed ") {
        Severity::Info
    } else if status.starts_with("No ") || status.starts_with("Nothing ") {
        Severity::Warn
    } else if status.starts_with("Exported ") || status.starts_with("Saved ") {
        Severity::Success
    } else if status.contains("cannot")
        || status.contains("already")
        || status.contains("at least")
        || status.contains(" is empty")
    {
        Severity::Warn
    } else {
        Severity::Info
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
    ///   never eat cap slots — and so the dedupe window
    ///   below is exactly the live window.
    /// - A push whose (text, severity) pair matches ANY live
    ///   toast does not stack a copy; it refreshes that
    ///   toast's birth time in place, so repeating a status
    ///   — consecutively or alternating with other ops —
    ///   keeps one toast per distinct status instead of
    ///   filling the stack with twins. The refresh is in
    ///   place on purpose: the toast keeps its items index,
    ///   so its `toast-{index}` key and dismiss button never
    ///   move under the pointer. Same text with a DIFFERENT
    ///   severity is a different status and stacks normally;
    ///   an expired twin is never refreshed — `expire` above
    ///   has already collected it, so the push starts a fresh
    ///   toast. (Wave 9: before, only a duplicate of the
    ///   NEWEST toast refreshed; non-consecutive twins
    ///   stacked.)
    /// - Past [`MAX_TOASTS`], the oldest toast is dropped.
    pub fn push(&mut self, text: impl Into<String>, severity: Severity, now: f32) {
        self.expire(now);
        let text = text.into();
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|t| t.text == text && t.severity == severity)
        {
            existing.born = now;
            return;
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
/// Every live toast carries a dismiss button; when at least
/// [`CLEAR_ALL_MIN`] toasts are live, a "Clear all" button
/// closes the stack and removes them all at once.
pub fn toasts_view(s: &State, now: f32) -> El {
    let t = tok::theme(true);
    let mut column = col()
        .key("toasts")
        .pin(None, None, Some(32.0), Some(48.0))
        .max_w(320.0)
        .gap(8.0)
        .items_end();
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
        column = column.child(icon_btn(
            "toast-clear-all",
            "trash-can",
            "Clear all",
            false,
            &t,
            move |s: &mut State| {
                s.toasts.dismiss_all(now);
            },
        ));
    }
    column
}

/// One toast card: a 3 pt severity bar at the leading edge,
/// then the severity glyph and the status text on the theme
/// card surface, fading over its last second. `index`
/// is the toast's items index — it keys the card (stable for
/// the toast's whole life, unlike a live position) and the
/// row's buttons dismiss by it. When `action` holds — this
/// is the newest live toast and the core can undo — the card
/// also carries the Undo button: pressing it undoes once and
/// dismisses this toast (the undo's own status change feeds
/// a fresh toast through the poll hook, as for any op).
/// Every card ends with an icon-only dismiss button
/// (`toast-dismiss-{index}`, an "xmark" glyph) that removes
/// exactly this toast and nothing else.
///
/// Phase N anatomy: the card is radius 12 with an 8 pt gap;
/// the bar is unkeyed and stretches to the card's full
/// height (the card row stretches its children, and the
/// glyph/text/action cluster keeps its own centring in an
/// inner row), so the severity reads before the glyph does.
/// The entrance is p15's M3: a 24 pt slide + fade on the
/// SETTLE spring pair, composing with the age fade (the
/// exit runs the same transition in reverse, then the
/// last-second opacity fade finishes it).
fn toast_row(toast: &Toast, index: usize, t: &Theme, now: f32, action: bool, undo_label: &str) -> El {
    let mut content = row()
        .center_y()
        .gap(8.0)
        .grow(1.0)
        .child(icon_el_rgb(
            toast.severity.icon(),
            13.0,
            toast.severity.icon_color(t),
        ))
        .child(
            text(toast.text.clone())
                .font(type_::LABEL)
                .color(t.text)
                .wrap_anywhere()
                .grow(1.0),
        );
    if action {
        content = content.child(icon_btn(
            "toast-undo",
            "arrow-rotate-left",
            undo_label,
            false,
            t,
            move |s: &mut State| {
                s.core.undo();
                s.toasts.dismiss(index);
            },
        ));
    }
    let dismiss_key = format!("toast-dismiss-{index}");
    content = content.child(icon_only_btn(
        &dismiss_key,
        "xmark",
        "Dismiss",
        26.0,
        false,
        t,
        move |s: &mut State| {
            s.toasts.dismiss(index);
        },
    ));
    row()
        .key(format!("toast-{index}"))
        .w(320.0)
        .max_w(320.0)
        .padding(12.0)
        .gap(8.0)
        .items_stretch()
        .radius(12.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .opacity(toast.fade(now))
        .transition(
            Transition::offset(24.0, 0.0)
                .combined(Transition::opacity())
                .springs(tok::motion::pair::SETTLE),
        )
        .child(
            el()
                .w(3.0)
                .radius(1.5)
                .fill(toast.severity.color(t)),
        )
        .child(content)
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
    fn push_non_consecutive_duplicate_refreshes_the_live_original() {
        // Wave 9: the dedupe window is the whole live stack,
        // not just the newest toast — alternating ops that
        // repeat a status refresh the original instead of
        // stacking a twin.
        let mut s = ToastsState::new();
        s.push("A", Severity::Info, 0.0);
        s.push("B", Severity::Info, 0.0);
        s.push("A", Severity::Info, 3.0);
        assert_eq!(s.items.len(), 2, "the repeated status must not stack a copy");
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            ["A", "B"],
            "the refresh is in place: order and items indices are stable"
        );
        assert_eq!(s.items[0].born, 3.0);
        assert_eq!(s.items[1].born, 0.0, "the refresh touches only the matching toast");
        // The refresh is real lifetime: at t=7.9 the
        // un-refreshed "B" (born 0, age 7.9 > TTL) is gone
        // while "A" (reborn at 3, age 4.9) still lives.
        let live: Vec<&str> = s.live(7.9).map(|t| t.text.as_str()).collect();
        assert_eq!(live, ["A"]);
    }

    #[test]
    fn push_duplicate_with_a_different_severity_stacks_even_non_consecutively() {
        // The dedupe key is the (text, severity) pair: the
        // same words meaning something else are a different
        // status, wherever in the window they land.
        let mut s = ToastsState::new();
        s.push("Export failed.", Severity::Info, 0.0);
        s.push("B", Severity::Info, 0.0);
        s.push("Export failed.", Severity::Error, 0.0);
        assert_eq!(s.items.len(), 3);
        assert_eq!(s.items[2].severity, Severity::Error);
        assert_eq!(s.items[0].born, 0.0, "the Info twin was not refreshed either");
    }

    #[test]
    fn push_after_the_twin_expired_starts_a_fresh_toast() {
        // "Within the live window" is literal: push expires
        // first, so a twin that is already dead is not
        // refreshed — the push starts a new toast, born now,
        // at the newest slot.
        let mut s = ToastsState::new();
        s.push("A", Severity::Info, 0.0);
        s.push("B", Severity::Info, 4.0);
        s.push("A", Severity::Info, 6.0); // the first "A" died at age > 5
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["B", "A"]);
        assert_eq!(s.items[1].born, 6.0);
    }

    #[test]
    fn push_caps_at_max_dropping_the_oldest() {
        let mut s = state_with(&["one", "two", "three"], 0.0);
        s.push("four", Severity::Info, 0.0);
        assert_eq!(s.items.len(), MAX_TOASTS);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["two", "three", "four"]);
    }

    #[test]
    fn push_dedupe_refresh_at_cap_drops_nothing() {
        let mut s = state_with(&["one", "two", "three"], 0.0);
        s.push("three", Severity::Info, 1.0);
        assert_eq!(s.items.len(), MAX_TOASTS);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["one", "two", "three"]);
        assert_eq!(s.items[2].born, 1.0);
    }

    #[test]
    fn push_refreshing_the_oldest_at_cap_drops_nothing() {
        // Wave 9: the refresh reaches every live toast, the
        // OLDEST included — and a refresh is never a push,
        // so at the cap nothing is dropped to make room
        // for it and nothing changes places.
        let mut s = state_with(&["one", "two", "three"], 0.0);
        s.push("one", Severity::Info, 2.0);
        assert_eq!(s.items.len(), MAX_TOASTS);
        let texts: Vec<&str> = s.items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["one", "two", "three"]);
        assert_eq!(s.items[0].born, 2.0);
        assert_eq!(s.items[1].born, 0.0);
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
        let mut s = state_with(&["one", "two", "three"], 0.0);
        // All three are stale by t=6; a push then must leave a
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
        // Both themes: the Wave 7 token wiring made color()
        // theme-aware, and the distinctness contract must hold
        // for the light *_on variants too, not only the dark
        // base values.
        for dark in [true, false] {
            let t = crate::rune_sec::app_theme::app_theme(dark);
            let colors = [
                Severity::Info.color(&t),
                Severity::Success.color(&t),
                Severity::Warn.color(&t),
                Severity::Error.color(&t),
            ];
            for (i, a) in colors.iter().enumerate() {
                for b in &colors[i + 1..] {
                    assert_ne!(a, b, "severity colours must not collide (dark={dark})");
                }
                assert_eq!(a.a, 1.0, "severity dots are fully opaque");
            }
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
        let mut s = state_with(&["one", "two", "three"], 0.0);
        assert_eq!(s.live_count(1.0), MAX_TOASTS);
        s.dismiss_all(1.0);
        assert_eq!(s.live_count(1.0), 0, "no toast stays live after Clear all");
        assert!(s.is_empty(), "all three were live, so nothing remains stored");
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

    // --- classify_status (Phase N) -------------------------------
    // Every string below is a real status from the tree
    // (grep-verified 2026-11-08) except the one precedence
    // probe, which is marked where it stands.

    #[test]
    fn classify_reads_the_engine_failure_shape_as_error() {
        // The `" failed"` shape, from the audio DSP ops and
        // their test suites.
        assert_eq!(classify_status("Audio fade failed: no clip 1."), Severity::Error);
        assert_eq!(
            classify_status("Audio compress failed: output would overwrite the source."),
            Severity::Error
        );
        assert_eq!(
            classify_status("Audio compress failed: no clip 3."),
            Severity::Error
        );
    }

    #[test]
    fn classify_reads_refusals_as_warn() {
        // The op did not happen; nothing is broken.
        assert_eq!(classify_status("No FX clip selected to split."), Severity::Warn);
        assert_eq!(classify_status("No camera key on this frame."), Severity::Warn);
        assert_eq!(classify_status("No saved file to reload."), Severity::Warn);
        assert_eq!(classify_status("Nothing copied yet."), Severity::Warn);
    }

    #[test]
    fn classify_reads_the_refusal_grammar_as_warn() {
        // Wave 6 (audit S18 §6 defect 2 / S18-11a): the
        // subject-first refusals the `"No "`/`"Nothing "`
        // prefixes miss. Every string is real (grep-verified
        // 2026-11-08; format strings rendered with the values
        // their own ops' tests assert). First, the eight
        // shapes the audit names:
        assert_eq!(
            classify_status("Frame 2 cannot be swapped with itself."),
            Severity::Warn
        );
        assert_eq!(classify_status("Clip 2 is already muted."), Severity::Warn);
        assert_eq!(
            classify_status("A movieclip needs at least one frame."),
            Severity::Warn
        );
        assert_eq!(classify_status("Keep at least one figure."), Severity::Warn);
        assert_eq!(
            classify_status("Lasso needs at least 3 points; nothing erased."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("Every frame in the range already has a key."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("3D library is empty — save a 3D figure first."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("First frame: cannot shift it earlier."),
            Severity::Warn
        );
        // Same-family siblings the same grammar arms catch:
        assert_eq!(
            classify_status("Last frame: cannot shift it later."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("Frame 11 already has a key on this channel."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("Bake animation needs at least 2 frames."),
            Severity::Warn
        );
        assert_eq!(classify_status("Figure name cannot be empty."), Severity::Warn);
        assert_eq!(classify_status("Project name cannot be empty."), Severity::Warn);
        assert_eq!(
            classify_status("Key at frame 1 cannot move earlier."),
            Severity::Warn
        );
        assert_eq!(classify_status("Clip 1 is already first."), Severity::Warn);
        assert_eq!(classify_status("Clip 1 is already last."), Severity::Warn);
        assert_eq!(classify_status("3D mode is already active."), Severity::Warn);
        assert_eq!(classify_status("Motion path already smooth."), Severity::Warn);
        assert_eq!(classify_status("Stroke 1 is already simple."), Severity::Warn);
        assert_eq!(
            classify_status("A library figure named \"demo\" already exists."),
            Severity::Warn
        );
        assert_eq!(
            classify_status("Pose 'idle' already has that name."),
            Severity::Warn
        );
        // Already Warn by its prefix; the grammar agrees, so
        // the prefix and grammar rules cannot drift apart on
        // this string.
        assert_eq!(
            classify_status(
                "Nothing to fill: the last stroke needs at least 3 points to make a shape."
            ),
            Severity::Warn
        );
    }

    #[test]
    fn classify_reads_the_normalise_no_op_as_info() {
        // Audit S18 §6 defect 3: the one false Warn. The
        // normalise sweep ran and found nothing to do — a
        // benign outcome, not a refusal (both peak values
        // the op's own tests produce).
        assert_eq!(
            classify_status("No audio clips needed normalising to peak 0.90."),
            Severity::Info
        );
        assert_eq!(
            classify_status("No audio clips needed normalising to peak 0.50."),
            Severity::Info
        );
        // The carve-out is the `" needed "` shape only:
        // sibling `"No "` refusals from the same panel stay
        // Warn.
        assert_eq!(classify_status("No audio clips to mix."), Severity::Warn);
        assert_eq!(
            classify_status("No audio clip selected to mute."),
            Severity::Warn
        );
    }

    #[test]
    fn classify_failure_reasons_wearing_refusal_words_stay_error() {
        // Near-miss precedence: both strings carry the
        // `" is empty"` grammar word inside a `" failed"`
        // reason. Error is checked first, so a failure
        // whose reason names an empty thing is still a
        // failure, never a mere refusal.
        assert_eq!(
            classify_status("Audio crossfade failed: a clip is empty."),
            Severity::Error
        );
        assert_eq!(
            classify_status("Audio trim failed: the trim range is empty."),
            Severity::Error
        );
    }

    #[test]
    fn classify_reads_export_and_save_confirmations_as_success() {
        assert_eq!(
            classify_status("Exported 5 PNG frames to ./export"),
            Severity::Success
        );
        assert_eq!(classify_status("Exported out/animation.apng."), Severity::Success);
        // Rendered from the library save's `"Saved {} to the
        // library."` format string.
        assert_eq!(classify_status("Saved Styk to the library."), Severity::Success);
    }

    #[test]
    fn classify_leaves_neutral_facts_as_info() {
        assert_eq!(classify_status("Undone."), Severity::Info);
        assert_eq!(classify_status("Redone."), Severity::Info);
        // The Success rule needs its trailing space: the
        // bare save acknowledgement is a neutral fact.
        assert_eq!(classify_status("Saved."), Severity::Info);
        // The push site never sends an empty status, but the
        // classifier must not panic on one.
        assert_eq!(classify_status(""), Severity::Info);
    }

    #[test]
    fn classify_error_outranks_the_refusal_prefix() {
        // Precedence probe: no current engine string carries
        // both shapes (grep-verified 2026-11-08), but the
        // rules are ordered so that if one ever does, the
        // failure wins over the refusal — a thing that
        // failed is not a mere "did not happen".
        assert_eq!(classify_status("No frames failed to render."), Severity::Error);
    }

    #[test]
    fn view_builds_with_a_classified_error_toast() {
        // The Phase N row construction (severity bar, inner
        // centred cluster, entrance transition) builds for a
        // classified toast exactly as for a hand-severitied
        // one; tree presence is the Runtime tests' job.
        let mut s = crate::init();
        let status = "Audio fade failed: no clip 1.";
        s.toasts.push(status, classify_status(status), 0.0);
        let _ = toasts_view(&s, 0.5);
    }
}
