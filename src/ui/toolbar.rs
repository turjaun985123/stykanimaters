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
//! Resting colours come from [`spec_colors`] (initially this
//! module's [`face_colors`], which remains for older callers).
//!
//! One deliberate difference: `btn()` has no danger state. Specs
//! add one — a destructive button (delete, clear, …) gets a dark
//! red resting background ([`BG_DANGER`]) so it reads as dangerous
//! before it is clicked. Danger wins over active when both are
//! set: the warning colour is never masked by the accent.
//!
//! Wave 14 generalises that one-off danger state into
//! [`BtnVariant`] — Primary / Secondary / Danger / Ghost — with
//! [`variant_colors`] as the single colour table. The variant is
//! now a spec's canonical styling: it *supersedes* the older
//! `danger` flag as the mechanism a caller reaches for. The flag
//! is kept for back-compatibility only, and a spec that still
//! carries `danger: true` is resolved to the Danger variant by
//! [`effective_variant`], so pre-variant specs render exactly as
//! dangerous as they always did.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_ELEVATED, C_ERROR, C_INK, C_MUTED, C_PANEL};

/// Resting background of an idle button: the literal `btn()` and
/// the kit's `info_chip` both use.
pub const BG_IDLE: u32 = 0x2C313A;

/// Resting background of a danger button. New with this module —
/// `btn()` has no equivalent. A dark red tint of the same weight
/// as [`BG_IDLE`], in the family of the toast error accent.
pub const BG_DANGER: u32 = 0x5A2C2C;

/// Background of an *active* danger button (the Danger variant in
/// its on state): the full toast error red, so an armed
/// destructive action reads hotter than a merely resting one.
pub const BG_DANGER_ACTIVE: u32 = C_ERROR;

/// The visual family a button belongs to.
///
/// This is the canonical styling field on [`ButtonSpec`]; it
/// supersedes the spec's legacy `danger` bool (see
/// [`effective_variant`] for how the two are reconciled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtnVariant {
    /// The call to action: accent background, white text, in both
    /// states — being "active" never changes it.
    Primary,
    /// The ordinary toolbar button: [`face_colors`]' palette
    /// (idle cell, accent when active). The default.
    Secondary,
    /// A destructive button: dark red at rest ([`BG_DANGER`]),
    /// full error red when active ([`BG_DANGER_ACTIVE`]).
    Danger,
    /// A quiet button that stays out of the way until engaged:
    /// panel background with muted text at rest, the kit's
    /// elevated surface with ink text when active.
    Ghost,
}

/// Resting `(background, foreground)` for a button of `variant`
/// in the given state. The one colour table every variant reads
/// from — [`button_face`] resolves a spec through here (via
/// [`spec_colors`]), so what a test pins here is what renders.
///
/// - Primary is `(C_ACCENT, white)` in both states: it is the
///   call to action, not a toggle.
/// - Secondary is exactly the pre-variant [`face_colors`]
///   palette, so existing toolbars look unchanged.
/// - Danger rests on [`BG_DANGER`] and heats up to
///   [`BG_DANGER_ACTIVE`] when active.
/// - Ghost rests on the panel itself (`C_PANEL` / `C_MUTED`) —
///   the closest a solid fill gets to transparent against a
///   panel — and lifts to `C_ELEVATED` / `C_INK` when active.
/// Hover is not covered here — faces always hover to `C_BORDER`,
/// exactly like `btn()`.
pub fn variant_colors(variant: BtnVariant, active: bool) -> (u32, u32) {
    match variant {
        BtnVariant::Primary => (C_ACCENT, 0xFFFFFF),
        BtnVariant::Secondary => {
            if active {
                (C_ACCENT, C_INK)
            } else {
                (BG_IDLE, C_INK)
            }
        }
        BtnVariant::Danger => {
            if active {
                (BG_DANGER_ACTIVE, C_INK)
            } else {
                (BG_DANGER, C_INK)
            }
        }
        BtnVariant::Ghost => {
            if active {
                (C_ELEVATED, C_INK)
            } else {
                (C_PANEL, C_MUTED)
            }
        }
    }
}

/// The variant a spec actually renders with.
///
/// `variant` is the canonical field and the one new code should
/// set; it supersedes the legacy `danger` flag as the styling
/// mechanism. For back-compatibility a spec carrying
/// `danger: true` — every pre-variant spec, and any hand-built
/// literal that sets the flag without touching `variant` — is
/// mapped to [`BtnVariant::Danger`] here, so the flag keeps its
/// old meaning instead of being silently dropped.
pub fn effective_variant(spec: &ButtonSpec) -> BtnVariant {
    if spec.danger {
        BtnVariant::Danger
    } else {
        spec.variant
    }
}

/// Resting `(background, foreground)` a spec renders with: its
/// [`effective_variant`] looked up in [`variant_colors`] at its
/// own active state. This is the exact pair [`button_face`]
/// paints, split out so tests can pin a spec's colours even
/// though a built `Div`'s style is not inspectable.
pub fn spec_colors(spec: &ButtonSpec) -> (u32, u32) {
    variant_colors(effective_variant(spec), spec.active)
}

/// Resting `(background, foreground)` for a button face.
///
/// Mirrors `btn()`'s palette: idle is [`BG_IDLE`], active is the
/// kit's `C_ACCENT`, and the foreground is always the kit's
/// `C_INK`. The danger background ([`BG_DANGER`]) takes
/// precedence over active. Hover is not covered here — faces
/// always hover to `C_BORDER`, exactly like `btn()`.
///
/// Kept for callers already written against the two-flag
/// palette; new code should read [`variant_colors`] (Secondary
/// is this palette, and Danger there additionally distinguishes
/// the active state).
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
    ///
    /// Legacy alias for `variant: BtnVariant::Danger`, kept so
    /// pre-variant specs keep working: when this is `true`,
    /// [`effective_variant`] maps the spec to Danger regardless
    /// of `variant`. New code should set `variant` instead —
    /// the variant field supersedes this flag.
    pub danger: bool,
    /// The button's visual family. Canonical styling field;
    /// defaults to [`BtnVariant::Secondary`] in [`ButtonSpec::new`].
    pub variant: BtnVariant,
}

impl ButtonSpec {
    /// A plain button: Secondary variant, inactive, not dangerous.
    pub fn new(label: impl Into<String>) -> Self {
        ButtonSpec {
            label: label.into(),
            active: false,
            danger: false,
            variant: BtnVariant::Secondary,
        }
    }

    /// A button of an explicit variant, inactive. The `danger`
    /// flag is kept in sync: it is set exactly when `variant` is
    /// [`BtnVariant::Danger`], so the two fields never contradict.
    pub fn with_variant(label: impl Into<String>, variant: BtnVariant) -> Self {
        ButtonSpec {
            danger: variant == BtnVariant::Danger,
            variant,
            ..ButtonSpec::new(label)
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
            variant: BtnVariant::Danger,
            ..ButtonSpec::new(label)
        }
    }
}

/// Render one spec as a button face: the element `btn()` builds,
/// styled identically (10px/6px padding, 6px radius, 13px text,
/// [`spec_colors`] resting colours, `C_BORDER` hover), but with
/// **no id and no click listener**. Attach both to the returned
/// `Div`, or wrap it, wherever the behaviour belongs.
pub fn button_face(spec: &ButtonSpec) -> Div {
    let (bg, fg) = spec_colors(spec);
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
    use super::{
        button_face, effective_variant, face_colors, spec_colors, toolbar_faces, variant_colors,
        BtnVariant, ButtonSpec, BG_DANGER, BG_DANGER_ACTIVE, BG_IDLE,
    };
    use crate::ui_kit::{C_ACCENT, C_ELEVATED, C_ERROR, C_INK, C_MUTED, C_PANEL};

    #[test]
    fn face_colors_idle_matches_btn_palette() {
        // btn(): bg 0x2C313A (= BG_IDLE), text in C_INK.
        assert_eq!(face_colors(false, false), (BG_IDLE, C_INK));
        assert_eq!(BG_IDLE, 0x2C313A);
    }

    #[test]
    fn face_colors_active_uses_accent() {
        assert_eq!(face_colors(true, false), (C_ACCENT, C_INK));
        // The Wave 14 kit retune moved the accent token.
        assert_eq!(C_ACCENT, 0x6C8CFF);
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
    fn variant_colors_primary_is_accent_and_white_in_both_states() {
        // The call to action never changes with the active flag.
        assert_eq!(variant_colors(BtnVariant::Primary, false), (C_ACCENT, 0xFFFFFF));
        assert_eq!(variant_colors(BtnVariant::Primary, true), (C_ACCENT, 0xFFFFFF));
    }

    #[test]
    fn variant_colors_secondary_matches_the_legacy_palette() {
        // Secondary is today's button_face colours, verbatim.
        assert_eq!(variant_colors(BtnVariant::Secondary, false), (BG_IDLE, C_INK));
        assert_eq!(variant_colors(BtnVariant::Secondary, true), (C_ACCENT, C_INK));
        assert_eq!(
            variant_colors(BtnVariant::Secondary, false),
            face_colors(false, false)
        );
        assert_eq!(
            variant_colors(BtnVariant::Secondary, true),
            face_colors(true, false)
        );
    }

    #[test]
    fn variant_colors_danger_heats_up_when_active() {
        assert_eq!(variant_colors(BtnVariant::Danger, false), (0x5A2C2C, C_INK));
        assert_eq!(variant_colors(BtnVariant::Danger, false), (BG_DANGER, C_INK));
        assert_eq!(variant_colors(BtnVariant::Danger, true), (C_ERROR, C_INK));
        assert_eq!(
            variant_colors(BtnVariant::Danger, true),
            (BG_DANGER_ACTIVE, C_INK)
        );
    }

    #[test]
    fn variant_colors_ghost_rests_on_the_panel_and_lifts_when_active() {
        assert_eq!(variant_colors(BtnVariant::Ghost, false), (C_PANEL, C_MUTED));
        assert_eq!(variant_colors(BtnVariant::Ghost, true), (C_ELEVATED, C_INK));
    }

    #[test]
    fn spec_constructors_set_their_flags() {
        let plain = ButtonSpec::new("Play");
        assert_eq!(plain.label, "Play");
        assert!(!plain.active);
        assert!(!plain.danger);
        // ButtonSpec::new defaults to the Secondary variant.
        assert_eq!(plain.variant, BtnVariant::Secondary);
        assert_eq!(effective_variant(&plain), BtnVariant::Secondary);

        let active = ButtonSpec::active("Onion");
        assert!(active.active);
        assert!(!active.danger);
        assert_eq!(active.variant, BtnVariant::Secondary);

        let danger = ButtonSpec::danger("Delete");
        assert!(danger.danger);
        assert!(!danger.active);
        // The danger constructor keeps flag and variant in sync.
        assert_eq!(danger.variant, BtnVariant::Danger);
        assert_eq!(effective_variant(&danger), BtnVariant::Danger);

        let primary = ButtonSpec::with_variant("Export", BtnVariant::Primary);
        assert_eq!(primary.variant, BtnVariant::Primary);
        assert!(!primary.danger);
        assert_eq!(spec_colors(&primary), (C_ACCENT, 0xFFFFFF));
    }

    #[test]
    fn legacy_danger_flag_resolves_to_danger_colours() {
        // A pre-variant literal: danger flag set, variant left at
        // the Secondary default. Back-compat maps it to Danger —
        // through the same spec_colors pair button_face paints.
        let legacy = ButtonSpec {
            label: "Delete".to_string(),
            active: false,
            danger: true,
            variant: BtnVariant::Secondary,
        };
        assert_eq!(effective_variant(&legacy), BtnVariant::Danger);
        assert_eq!(spec_colors(&legacy), variant_colors(BtnVariant::Danger, false));
        assert_eq!(spec_colors(&legacy), (BG_DANGER, C_INK));
        // The face itself still constructs (no window needed).
        let _face = button_face(&legacy);

        // And in its on state it resolves to the active danger red.
        let legacy_active = ButtonSpec { active: true, ..legacy };
        assert_eq!(
            spec_colors(&legacy_active),
            variant_colors(BtnVariant::Danger, true)
        );
        assert_eq!(spec_colors(&legacy_active), (BG_DANGER_ACTIVE, C_INK));
        let _face = button_face(&legacy_active);
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
