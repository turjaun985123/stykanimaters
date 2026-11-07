//! Toast notifications for the editor.
//!
//! The editor historically showed feedback as a single status string
//! that each new message overwrote. This module provides the
//! replacement: a small queue of transient messages ([`ToastQueue`])
//! plus a renderer ([`toast_stack`]) that draws them as a stack of
//! pills, newest last.
//!
//! The queue is plain data with no editor knowledge and no handlers.
//! The owner pushes messages as events happen and calls
//! [`ToastQueue::tick`] once per UI tick (the app ticks every 33 ms);
//! toasts expire on their own after [`DEFAULT_TTL_TICKS`] ticks
//! (≈ 4 s). At most [`MAX_TOASTS`] toasts are visible at once, so a
//! burst of messages can never flood the screen.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK, C_MUTED, C_PANEL};

/// Accent colour for error toasts.
pub const C_ERROR: u32 = 0xB23A3A;

/// How long a toast stays visible, in UI ticks. At the app's 33 ms
/// tick this is ≈ 4 s.
pub const DEFAULT_TTL_TICKS: u32 = 120;

/// Maximum number of toasts shown at once; older ones are dropped.
pub const MAX_TOASTS: usize = 3;

/// The severity of a toast, which picks its accent colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

impl ToastKind {
    /// The accent colour marking this kind (the dot on the pill):
    /// muted for info, the app accent for success, red for errors.
    pub fn accent(self) -> u32 {
        match self {
            ToastKind::Info => C_MUTED,
            ToastKind::Success => C_ACCENT,
            ToastKind::Error => C_ERROR,
        }
    }
}

/// One transient message. `ttl_ticks` counts down on every
/// [`ToastQueue::tick`]; the toast is dropped when it reaches zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub ttl_ticks: u32,
}

impl Toast {
    /// A toast with the default time-to-live.
    pub fn new(kind: ToastKind, text: impl Into<String>) -> Self {
        Toast {
            kind,
            text: text.into(),
            ttl_ticks: DEFAULT_TTL_TICKS,
        }
    }
}

/// A capped stack of toasts, oldest first, newest last.
#[derive(Clone, Debug, Default)]
pub struct ToastQueue {
    items: Vec<Toast>,
}

impl ToastQueue {
    pub fn new() -> Self {
        ToastQueue { items: Vec::new() }
    }

    /// The queued toasts, oldest first.
    pub fn items(&self) -> &[Toast] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Queue a message with the default time-to-live.
    ///
    /// If the newest toast already shows the same text, it is not
    /// duplicated: its kind is updated and its timer restarted, so a
    /// repeated event just keeps its toast alive. When the push
    /// exceeds [`MAX_TOASTS`], the oldest toasts are dropped.
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        if let Some(last) = self.items.last_mut() {
            if last.text == text {
                last.kind = kind;
                last.ttl_ticks = DEFAULT_TTL_TICKS;
                return;
            }
        }
        self.items.push(Toast::new(kind, text));
        while self.items.len() > MAX_TOASTS {
            self.items.remove(0);
        }
    }

    /// Advance every toast's timer by one UI tick and drop the ones
    /// whose time has run out.
    pub fn tick(&mut self) {
        for toast in &mut self.items {
            toast.ttl_ticks = toast.ttl_ticks.saturating_sub(1);
        }
        self.items.retain(|toast| toast.ttl_ticks > 0);
    }

    /// Drop every queued toast immediately.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// The toast stack as a vertical column of pills, newest last. An
/// empty queue renders as an empty column, so the caller can place
/// the result unconditionally.
pub fn toast_stack(queue: &ToastQueue) -> Div {
    let mut stack = div().flex_col().gap(px(4.0));
    for toast in queue.items() {
        stack = stack.child(toast_row(toast));
    }
    stack
}

/// One pill: panel background, thin border, a coloured dot in the
/// kind's accent colour, and the message in 12px ink.
fn toast_row(toast: &Toast) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .child(
            div()
                .w(px(8.0))
                .h(px(8.0))
                .rounded(px(999.0))
                .bg(rgb(toast.kind.accent())),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(toast.text.clone()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(queue: &ToastQueue) -> Vec<&str> {
        queue.items().iter().map(|t| t.text.as_str()).collect()
    }

    #[test]
    fn push_caps_at_max_toasts_dropping_oldest() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "one");
        queue.push(ToastKind::Info, "two");
        queue.push(ToastKind::Info, "three");
        queue.push(ToastKind::Info, "four");
        assert_eq!(queue.len(), MAX_TOASTS);
        assert_eq!(texts(&queue), ["two", "three", "four"]);
    }

    #[test]
    fn push_dedupes_identical_consecutive_text() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "saved");
        queue.push(ToastKind::Success, "saved");
        assert_eq!(queue.len(), 1);
        // The newest kind wins on a repeat.
        assert_eq!(queue.items()[0].kind, ToastKind::Success);
        // The same text is only a duplicate while it is consecutive.
        queue.push(ToastKind::Info, "other");
        queue.push(ToastKind::Info, "saved");
        assert_eq!(texts(&queue), ["saved", "other", "saved"]);
    }

    #[test]
    fn dedupe_restarts_the_timer() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "saved");
        for _ in 0..(DEFAULT_TTL_TICKS - 1) {
            queue.tick();
        }
        assert_eq!(queue.items()[0].ttl_ticks, 1);
        queue.push(ToastKind::Info, "saved");
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.items()[0].ttl_ticks, DEFAULT_TTL_TICKS);
    }

    #[test]
    fn tick_expires_a_toast_after_its_ttl() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "hello");
        for _ in 0..(DEFAULT_TTL_TICKS - 1) {
            queue.tick();
        }
        assert_eq!(queue.len(), 1);
        queue.tick();
        assert!(queue.is_empty());
    }

    #[test]
    fn tick_drops_only_the_expired_toasts() {
        let mut queue = ToastQueue {
            items: vec![
                Toast {
                    kind: ToastKind::Info,
                    text: "old".into(),
                    ttl_ticks: 1,
                },
                Toast {
                    kind: ToastKind::Error,
                    text: "fresh".into(),
                    ttl_ticks: 5,
                },
            ],
        };
        queue.tick();
        assert_eq!(texts(&queue), ["fresh"]);
        assert_eq!(queue.items()[0].ttl_ticks, 4);
    }

    #[test]
    fn ordering_is_oldest_first_newest_last() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "first");
        queue.push(ToastKind::Success, "second");
        queue.push(ToastKind::Error, "third");
        assert_eq!(texts(&queue), ["first", "second", "third"]);
        assert_eq!(queue.items()[2].kind, ToastKind::Error);
    }

    #[test]
    fn clear_drops_everything() {
        let mut queue = ToastQueue::new();
        queue.push(ToastKind::Info, "one");
        queue.push(ToastKind::Info, "two");
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn kind_accents_are_distinct() {
        assert_eq!(ToastKind::Info.accent(), C_MUTED);
        assert_eq!(ToastKind::Success.accent(), C_ACCENT);
        assert_eq!(ToastKind::Error.accent(), C_ERROR);
        assert_eq!(C_ERROR, 0xB23A3A);
    }

    #[test]
    fn empty_queue_renders_an_empty_stack() {
        let queue = ToastQueue::new();
        // Construction must not panic and the queue stays untouched.
        let _stack = toast_stack(&queue);
        assert!(queue.is_empty());
    }
}
