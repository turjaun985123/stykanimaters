//! Data-driven button faces for toolbars.
//!
//! `crate::app`'s private `btn()` helper builds every toolbar
//! button in the editor: it takes an id, a label, an active flag
//! and a click listener, and returns the styled element. That
//! fuses two halves — how a button *looks* and what it *does* —
//! so a toolbar can only be assembled by hand-writing one `btn()`
//! call per button.
//!
//! This module is the visual half, split out so toolbars can be
//! built from data: a [`ButtonSpec`] describes a button, and
//! [`button_face`] renders exactly the element `btn()` renders,
//! minus the parts that wire it up. The caller keeps ownership of
//! behaviour — attach `.id(...)` and `.on_click(...)` to the
//! returned `Div` (both work directly on it), or wrap it — while
//! this module owns the pixels. [`toolbar_faces`] maps a whole
//! spec list to faces in one call.
//!
//! Styling mirrors `btn()` exactly: 10px/6px padding, 6px radius,
//! 13px ink text, pointer cursor, hover background `C_BORDER`.
//! Resting colours come from [`face_colors`].
//!
//! One deliberate difference: `btn()` has no danger state. Specs
//! add one — a destructive button (delete, clear, …) gets a dark
//! red resting background ([`BG_DANGER`]) so it reads as dangerous
//! before it is clicked. Danger wins over active when both are
//! set: the warning colour is never masked by the accent.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK};

/// Resting background of an idle button: the literal `btn()` and
/// the kit's `info_chip` both use.
pub const BG_IDLE: u32 = 0x2C313A;

/// Resting background of a danger button. New with this module —
/// `btn()` has no equivalent. A dark red tint of the same weight
/// as [`BG_IDLE`], in the family of the toast error accent.
pub const BG_DANGER: u32 = 0x5A2C2C;

/// Resting `(background, foreground)` for a button face.
///
/// Mirrors `btn()`'s palette: idle is [`BG_IDLE`], active is the
/// kit's `C_ACCENT`, and the foreground is always the kit's
/// `C_INK`. The danger background ([`BG_DANGER`]) takes
/// precedence over active. Hover is not covered here — faces
/// always hover to `C_BORDER`, exactly like `btn()`.
pub fn face_colors(active: bool, danger: bool) -> (u32, u32) {
    let bg = if danger {
        BG_DANGER
    } else if active {
        C_ACCENT
    } else {
        BG_IDLE
    };
    (bg, C_INK)
}

/// One button described as data: its label and visual state.
///
/// Behaviour is deliberately absent — no id, no listener. Build
/// specs in a list, render them with [`toolbar_faces`] (or a
/// single one with [`button_face`]), then wire each returned
/// `Div` up where the editor state lives.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonSpec {
    /// Text shown on the button.
    pub label: String,
    /// Accent background: the button's mode is currently on.
    pub active: bool,
    /// Danger background: the button does something destructive.
    pub danger: bool,
}

impl ButtonSpec {
    /// A plain button: inactive, not dangerous.
    pub fn new(label: impl Into<String>) -> Self {
        ButtonSpec {
            label: label.into(),
            active: false,
            danger: false,
        }
    }

    /// A button shown in its on state (accent background).
    pub fn active(label: impl Into<String>) -> Self {
        ButtonSpec {
            active: true,
            ..ButtonSpec::new(label)
        }
    }

    /// A destructive button (danger background).
    pub fn danger(label: impl Into<String>) -> Self {
        ButtonSpec {
            danger: true,
            ..ButtonSpec::new(label)
        }
    }
}

/// Render one spec as a button face: the element `btn()` builds,
/// styled identically (10px/6px padding, 6px radius, 13px text,
/// [`face_colors`] resting colours, `C_BORDER` hover), but with
/// **no id and no click listener**. Attach both to the returned
/// `Div`, or wrap it, wherever the behaviour belongs.
pub fn button_face(spec: &ButtonSpec) -> Div {
    let (bg, fg) = face_colors(spec.active, spec.danger);
    div()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .bg(rgb(bg))
        .hover(|s| s.bg(rgb(C_BORDER)))
        .text_size(px(13.0))
        .text_color(rgb(fg))
        .child(spec.label.clone())
}

/// Render a list of specs as button faces, in order. The faces
/// are unwired (see [`button_face`]); lay them out in a
/// [`crate::ui_kit::group_row`] or any other container.
pub fn toolbar_faces(specs: &[ButtonSpec]) -> Vec<Div> {
    specs.iter().map(button_face).collect()
}

#[cfg(test)]
mod tests {
    use super::{button_face, face_colors, toolbar_faces, ButtonSpec, BG_DANGER, BG_IDLE};
    use crate::ui_kit::{C_ACCENT, C_INK};

    #[test]
    fn face_colors_idle_matches_btn_palette() {
        // btn(): bg 0x2C313A, text 0xE8E8EC (= C_INK).
        assert_eq!(face_colors(false, false), (BG_IDLE, C_INK));
        assert_eq!(BG_IDLE, 0x2C313A);
    }

    #[test]
    fn face_colors_active_uses_accent() {
        assert_eq!(face_colors(true, false), (C_ACCENT, C_INK));
        assert_eq!(C_ACCENT, 0x3D5A99);
    }

    #[test]
    fn face_colors_danger_uses_danger_tint() {
        assert_eq!(face_colors(false, true), (BG_DANGER, C_INK));
        assert_eq!(BG_DANGER, 0x5A2C2C);
    }

    #[test]
    fn face_colors_danger_wins_over_active() {
        assert_eq!(face_colors(true, true), (BG_DANGER, C_INK));
    }

    #[test]
    fn spec_constructors_set_their_flags() {
        let plain = ButtonSpec::new("Play");
        assert_eq!(plain.label, "Play");
        assert!(!plain.active);
        assert!(!plain.danger);

        let active = ButtonSpec::active("Onion");
        assert!(active.active);
        assert!(!active.danger);

        let danger = ButtonSpec::danger("Delete");
        assert!(danger.danger);
        assert!(!danger.active);
    }

    #[test]
    fn toolbar_faces_renders_one_face_per_spec() {
        let specs = [
            ButtonSpec::new("Play"),
            ButtonSpec::active("Onion"),
            ButtonSpec::danger("Delete"),
        ];
        assert_eq!(toolbar_faces(&specs).len(), 3);
        assert!(toolbar_faces(&[]).is_empty());
        // A single face constructs without panicking (no window
        // needed — a Div is inert data until rendered).
        let _face = button_face(&specs[0]);
    }
}
