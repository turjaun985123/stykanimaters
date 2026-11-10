//! App bar, transport, timeline and status builders for the
//! Rune editor shell (Phase C of the GPUI→Rune migration).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Every control here calls exactly one real `EditorCore` op
//! (or, for Play/Pause, flips the same public core field the
//! GPUI app's button flipped — there is no op for it; see
//! `editor.rs` field docs). Element keys carry the old GPUI
//! control ids where one existed, so headless `--tree` checks
//! can find them. Phase F removed the app bar's + Figure and
//! Onion buttons: they duplicated the figures panel's own
//! add-figure button and the onion controls panel's
//! onion-toggle, so those panels are the single homes now.
//!
//! Labels: since Phase E the app-bar and transport labels
//! resolve through `rune_sec::i18n_wire::tr` against the
//! engine's current language (`core.lang`), so they follow the
//! language picker. One label takes the table's canonical text
//! rather than Phase C's shorter literal: the delete-frame
//! button shows `delete_frame` ("Delete"). The timeline chips
//! are bare frame numbers and stay untranslated. `status_text`
//! deliberately keeps its literal English format — see its own
//! doc comment.
//!
//! Phase N (foundation F2) completed the button family in
//! this file: `btn_inert` and `btn_muted_live` give the text
//! button its two disabled forms (P05 §4's rule — inert
//! drops the handler, muted-live keeps it so the press can
//! deliver a refusal status; both render the resting card
//! fill with secondary ink), and `danger_btn` is the
//! family's one danger member, on the deepened reds P12
//! measured for it. `btn` also gained Rune's `press_scale`
//! press feedback (P15 M1).
//!
//! UI mission wave 6: every member of the family wires its
//! label as the element's help text (Rune's native
//! tooltip), under the convention documented in
//! `rune_sec::icon_btn`'s module docs — a builder that
//! receives a human label passes it to the help setter, so
//! call sites inherit the tooltip by construction. This
//! file had to move with icon_btn.rs: that file's
//! unknown-icon fallbacks delegate to `btn` / `danger_btn`
//! here, and the tooltip must survive the degrade.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::app_theme;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn, icon_btn_inert};
use crate::rune_sec::project_tools::session_dirty;
use crate::{ConfirmAction, State};

/// One chrome button: a padded, rounded label with hover/press
/// fills; `active` renders it in the accent colour (toggle
/// state). The press handler is attached here, as in the GPUI
/// app's `btn()` helper.
///
/// Phase N (P15 M1) added `.press_scale(0.97)`: the box
/// shrinks slightly while a press is held and springs back
/// on release. Layout and hit testing keep the resting
/// size, and the host's Reduce Motion setting turns it off.
pub fn btn(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    el()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .press_scale(0.97)
        .on_press(f)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if active { Color::WHITE } else { t.text }),
        )
}

/// The inert text button (Phase N, P05 §5.2): `btn`'s box
/// and resting fill, secondary ink, and **no press handler
/// at all** — a plain element, not a control, so a stray
/// click cannot fire a refusal status or an undo snapshot.
/// For controls whose op would no-op from the current
/// state and has nothing to say about it. This is the
/// element `icon_btn_inert`'s unknown-icon fallback always
/// hand-rolled; naming it here gives the family one
/// implementation of the inert text box.
pub fn btn_inert(key: &str, label: &str, t: &Theme) -> El {
    el()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(t.card)
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
}

/// The muted-live text button (Phase N, P05 §4/§5.2): the
/// inert visual — resting card fill, secondary ink, no
/// hover/press fills, no press feedback — with the press
/// handler KEPT. This is frame_ops' `fbtn` muted branch
/// promoted to a family member: the gated op still runs
/// and its refusal status ("First frame: cannot shift it
/// earlier.") is the designed feedback, so the status
/// line answers instead of the button flashing. Use only
/// where a test pins that refusal status; where the op
/// has nothing to say, use [`btn_inert`] and drop the
/// handler.
///
/// `active` keeps `fbtn`'s precedence: an ON toggle is
/// not muted at all and renders exactly as `btn`'s active
/// state (the muted gate applies to the off state). The
/// two flags cannot both bite in practice — frame_ops'
/// active sites are the loop endpoints, whose mute
/// condition is an empty timeline, which cannot carry a
/// loop.
pub fn btn_muted_live(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    if active {
        return btn(key, label, true, t, f);
    }
    el()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(t.card)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
}

/// The family's danger reds (Phase N, P12's measured fix).
/// White LABEL text on the old local copies' `0xF16F78`
/// measured ≈ 2.9:1 — under even the 3:1 floor, and its
/// darker hover was paradoxically the *more* legible
/// state. These deepened reds put the resting fill at
/// ≈ 5:1 against white, with hover/press deeper still.
/// Fixed names and values: `icon_btn`'s `danger_icon_btn`
/// imports them from here so the text and icon danger
/// members can never drift apart again (the two local
/// copies this replaces had already drifted in shape).
pub const DANGER_FILL: u32 = 0xC93A44;
/// Hover step of [`DANGER_FILL`].
pub const DANGER_HOVER: u32 = 0xB2323E;
/// Press step of [`DANGER_FILL`].
pub const DANGER_PRESS: u32 = 0x9C2934;

/// A `0xRRGGBB` danger constant as a Rune colour, via the
/// shell's hex idiom (the same conversion
/// `stage_view::hex_color` performs). The fallback is
/// unreachable — the format is always a valid 6-digit
/// hex literal — and exists only because the parser is
/// fallible in general.
fn danger_color(rgb: u32) -> Color {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF)).unwrap_or(Color::WHITE)
}

/// The family's one danger button (Phase N, P05 §5.2):
/// `btn`'s metrics and white LABEL ink on the danger
/// fills ([`DANGER_FILL`] / [`DANGER_HOVER`] /
/// [`DANGER_PRESS`]), always in the danger state — there
/// is no inactive or inert form, matching the local
/// copies it retires (inspector_figure's and
/// inspector_segment's `danger_btn`, whose deliberate
/// danger styling this preserves at a legible depth).
/// Like `btn` it carries the family's `press_scale`.
pub fn danger_btn(
    key: &str,
    label: &str,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    el()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(danger_color(DANGER_FILL))
        .hover(danger_color(DANGER_HOVER))
        .press(danger_color(DANGER_PRESS))
        .press_scale(0.97)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(Color::WHITE))
}

/// Play/pause: flips `core.playing` (the GPUI app's
/// `toggle_play` did exactly this field flip; audio start is a
/// later phase). Starting playback syncs the playhead to the
/// shown frame, the same sync `EditorCore::goto` performs, so
/// `advance_play` starts from the frame on screen.
pub fn toggle_play(s: &mut State) {
    if !s.core.playing {
        s.core.playhead = s.core.current as f32;
    }
    s.core.playing = !s.core.playing;
}

/// The app bar's modified marker (Phase N, charter §15):
/// an 8 pt accent dot shown after the project name while
/// the project is dirty, absent while it is clean. The
/// signal is the same one the status strip's project
/// segment reads (see `status_strip::project_text`): the
/// core's public edit counter — dirty exactly when
/// `core.edits_since_autosave() > 0`, i.e. edits the
/// autosave has not captured yet. Not the recovery file's
/// mtime: that is the autosave segment's separate signal,
/// and the two must not be conflated.
///
/// UI mission Wave 9: the fill is the app palette's
/// accent for the CURRENT theme ([`dirty_dot_fill`]),
/// not Rune's `tok::accent(0, true)` — that call
/// hard-codes the dark appearance, so in the light theme
/// the dot painted the dark accent while the rest of the
/// shell wore the light one (the same defect class Wave 8
/// fixed in the status strip).
fn dirty_dot(core: &EditorCore, t: &Theme) -> Option<El> {
    if core.edits_since_autosave() > 0 {
        Some(el().size(8.0, 8.0).radius(4.0).fill(dirty_dot_fill(t)))
    } else {
        None
    }
}

/// The dot's colour seam (the `strip_colors` pattern
/// from Wave 8): a named, unit-testable binding from
/// the theme to the one colour the dot paints, so the
/// test below can pin it without reading an element
/// tree (which unit tests cannot do).
fn dirty_dot_fill(t: &Theme) -> Color {
    app_theme::accent(t)
}

/// The app bar Load button's press (UI mission Wave 4,
/// audit S16 D2): Load replaces the whole session with
/// the working file's contents (`core.load()`), so it
/// runs through the crate-level [`ConfirmAction`] gate
/// as an `OpenProject` action — while the session is
/// dirty ([`session_dirty`]) the first press only arms
/// and the button re-renders as its danger form (see
/// [`app_bar`]); the confirming press — or any press
/// while clean — runs the load exactly as before and
/// clears the arm. The armed label is deliberately not
/// routed through the i18n table: the table has no
/// discard-confirmation string, and inventing a key
/// would break the census the i18n checks pin — the
/// panels' confirm labels are plain literals for the
/// same reason.
pub fn load_pressed(s: &mut State) {
    if s.confirm_action != Some(ConfirmAction::OpenProject) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::OpenProject);
        s.core.status =
            "Loading would discard unsaved changes — press Load again to confirm.".into();
        return;
    }
    s.confirm_action = None;
    s.core.load();
}

/// Top bar: project name, then Save / Load / Undo / Redo —
/// each wired to its core op. Labels come from the i18n table
/// (keys `save`, `load`, `undo`, `redo`) in the core's current
/// language. (Phase F: the + Figure and Onion buttons that
/// used to sit here were removed — they duplicated the
/// figures panel's add-figure button and the onion panel's
/// onion-toggle.)
///
/// UI mission Wave 4 (audit S16 D2): Load is a session
/// replace, so its press is gated — see [`load_pressed`].
/// While an `OpenProject` discard is armed (from here or
/// from any other gated Open site), the Load button
/// renders as [`danger_icon_btn`] "Discard changes?"
/// under the same `load` key; the confirming press is
/// [`load_pressed`] again.
///
/// UI mission Wave 3 (audit S04 D2/D4): the File/Edit menus
/// no longer re-mount these four commands, so this bar is
/// their single mount — and Undo/Redo render honestly: each
/// goes through its `icon_btn_inert` twin while its stack
/// is empty (`core.undo()` / `core.redo()` on an empty
/// stack are silent no-ops, so a live-looking button would
/// lie), and the live `icon_btn` once the core's own depth
/// counters (`undo_depth()` / `redo_depth()`) say it can
/// run. Key, glyph, label and the live handler are
/// unchanged in both forms — the layers panel's
/// live/inert idiom.
///
/// Phase N (charter §15): the name (TITLE, as before) now
/// rides in a growing cluster with [`dirty_dot`] — the
/// accent dot appears after the name while the project is
/// modified. §15's hairline divider under the bar +
/// workspace row is deliberately NOT drawn here: the app
/// bar, the menu bar and the workspace row are separate
/// root children in main.rs, so a divider inside this
/// builder would land between the app bar and the menu
/// bar — the wrong place. That divider is an integrator
/// layout item, recorded in the F2 report.
pub fn app_bar(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let mut name_row = row()
        .gap(6.0)
        .center_y()
        .grow(1.0)
        .child(
            text(core.project.name.clone())
                .font(type_::TITLE)
                .color(t.text),
        );
    if let Some(dot) = dirty_dot(core, t) {
        name_row = name_row.child(dot);
    }
    // Undo/Redo, depth-gated (see this fn's docs): the
    // inert twin drops the handler, so an empty-stack
    // press cannot even reach the core's silent no-op.
    let undo_btn = if core.undo_depth() == 0 {
        icon_btn_inert("undo", "arrow-rotate-left", &i18n_wire::tr(core, "undo"), t)
    } else {
        icon_btn("undo", "arrow-rotate-left", &i18n_wire::tr(core, "undo"), false, t, |s| {
            s.core.undo();
            // Wave 10 (C6): history navigation can change
            // the stroke list under the selection index —
            // the selection never survives an undo/redo.
            s.stroke_selected = None;
        })
    };
    let redo_btn = if core.redo_depth() == 0 {
        icon_btn_inert("redo", "arrow-rotate-right", &i18n_wire::tr(core, "redo"), t)
    } else {
        icon_btn("redo", "arrow-rotate-right", &i18n_wire::tr(core, "redo"), false, t, |s| {
            s.core.redo();
            s.stroke_selected = None;
        })
    };
    row()
        .key("app-bar")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(name_row)
        .child(icon_btn("save", "floppy-disk", &i18n_wire::tr(core, "save"), false, t, |s| {
            s.core.save()
        }))
        .child(if s.confirm_action == Some(ConfirmAction::OpenProject) {
            danger_icon_btn("load", "folder-open", "Discard changes?", false, t, load_pressed)
        } else {
            icon_btn("load", "folder-open", &i18n_wire::tr(core, "load"), false, t, load_pressed)
        })
        .child(undo_btn)
        .child(redo_btn)
}

/// Transport + frame ops row: Prev / Play-Pause / Next (frame
/// stepping wraps, as the GPUI key handler did), then Add /
/// Duplicate / Delete frame. Labels come from the i18n table
/// (keys `prev_frame`, `play`/`pause`, `next_frame`,
/// `add_frame`, `duplicate_frame`, `delete_frame`) in the
/// core's current language. (Phase N, P06: Duplicate and
/// Delete are `icon_btn`s now — `copy` / `trash-can` glyphs,
/// the same dialect as the rest of the row and as the
/// Frames section's counterparts; keys, labels, handlers
/// and active flags unchanged.)
///
/// Wave 10 (audit E9): Prev/Next render through their
/// `icon_btn_inert` twins on a zero-frame project — the
/// live handlers guard on `n > 0` and do literally
/// nothing there, no status included, so a live-looking
/// button would lie (the app bar's Undo/Redo depth-gating
/// idiom, applied to the frame count). Delete stays live
/// at every count: `delete_frame` answers a one-frame
/// press with its refusal status ("Cannot delete the
/// only frame."), which is the designed feedback — the
/// `btn_muted_live` class, not a silent no-op. Play stays
/// live too: it flips real state the strip reports. Add
/// and Duplicate stay live on an empty project — Add is
/// the recovery action the timeline's empty state names —
/// but note the core defect that makes that load-bearing:
/// `EditorCore::add_frame` indexes `frames[current]`
/// unguarded, so on a zero-frame project Add/Duplicate
/// would panic in the core, not no-op; the guard belongs
/// in the core op (engine-side, flagged in the E9
/// report), not in a chrome gate that would strand the
/// empty state's one named action.
pub fn transport_row(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let play_label = i18n_wire::tr(core, if core.playing { "pause" } else { "play" });
    // Prev/Next, frame-count-gated (see this fn's docs):
    // the inert twin drops the handler, so an empty-project
    // press cannot even reach the guarded no-op.
    let prev_btn = if core.frame_count() == 0 {
        icon_btn_inert("prev", "chevron-left", &i18n_wire::tr(core, "prev_frame"), t)
    } else {
        icon_btn(
            "prev",
            "chevron-left",
            &i18n_wire::tr(core, "prev_frame"),
            false,
            t,
            |s| {
                let n = s.core.frame_count();
                if n > 0 {
                    s.core.goto((s.core.current + n - 1) % n);
                }
            },
        )
    };
    let next_btn = if core.frame_count() == 0 {
        icon_btn_inert("next", "chevron-right", &i18n_wire::tr(core, "next_frame"), t)
    } else {
        icon_btn(
            "next",
            "chevron-right",
            &i18n_wire::tr(core, "next_frame"),
            false,
            t,
            |s| {
                let n = s.core.frame_count();
                if n > 0 {
                    s.core.goto((s.core.current + 1) % n);
                }
            },
        )
    };
    row()
        .key("transport")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(prev_btn)
        .child(icon_btn(
            "play",
            if core.playing { "pause" } else { "play" },
            &play_label,
            core.playing,
            t,
            |s| toggle_play(s),
        ))
        .child(next_btn)
        .child(el().grow(1.0))
        .child(icon_btn(
            "add-frame",
            "plus",
            &i18n_wire::tr(core, "add_frame"),
            false,
            t,
            |s| s.core.add_frame(),
        ))
        .child(icon_btn(
            "dup-frame",
            "copy",
            &i18n_wire::tr(core, "duplicate_frame"),
            false,
            t,
            |s| s.core.duplicate_frame(),
        ))
        .child(icon_btn(
            "del-frame",
            "trash-can",
            &i18n_wire::tr(core, "delete_frame"),
            false,
            t,
            |s| s.core.delete_frame(),
        ))
}

/// The timeline: one numbered chip per frame in a horizontally
/// scrolling row; the current frame is accent-filled. A chip
/// press goes to that frame (`EditorCore::goto`).
///
/// Wave 10 (audit E9): this is the LEGACY Phase C builder —
/// `main.rs` mounts `timeline_marks::timeline_marks` in this
/// slot, and nothing calls this function. It stays because
/// its keys (`timeline`, `frame-N`) are the census baseline
/// the richer builder was built to match, and removing it
/// would read as a key loss; but do not budget the timeline
/// card's height against this row — the mounted card's
/// stack (ruler, chips, marker row, clips lane, loop row)
/// lives in `timeline_marks.rs`.
pub fn timeline_row(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let accent = tok::accent(0, true);
    let mut chips = row().key("timeline").gap(4.0);
    for i in 0..core.frame_count() {
        let active = i == core.current;
        chips = chips.child(
            el()
                .key(format!("frame-{}", i + 1))
                .size(30.0, 24.0)
                .radius(6.0)
                .fill(if active { accent } else { t.card })
                .hover(if active { accent } else { t.control_track })
                // Wave P (P4): the press half of the `btn`
                // state table (active → accent, inactive →
                // `control_off`); the chips carried hover
                // only. This builder is unmounted (see the
                // fn docs) — the fix keeps the retained
                // census baseline honest, not live UI.
                .press(if active { accent } else { t.control_off })
                .center()
                .on_press(move |s: &mut State| s.core.goto(i))
                .child(
                    text(format!("{}", i + 1))
                        .font(type_::LABEL)
                        .color(if active { Color::WHITE } else { t.text }),
                ),
        );
    }
    chips.scroll_x().w_full()
}

/// The status line text: `Frame {cur+1}/{count} · {fps} fps ·
/// Zoom {pct}% · {core.status}` — the same fields the GPUI
/// status strip showed (zoom is the camera key zoom at the
/// current frame, via `EditorCore::camera_here`).
///
/// Phase E decision: this format stays literal English. The
/// `frame` and `zoom` words do exist in the i18n table, but
/// the `fps` entry is the label "FPS" (an abbreviation), while
/// this line uses lowercase "fps" as a unit suffix after the
/// number — dropping the table value in would change the
/// rendered text and break the format this function's tests
/// pin. The segmented `status_strip` section is the shell's
/// live status surface; this line remains as the compact
/// fallback it was in Phase C.
pub fn status_text(core: &EditorCore) -> String {
    let fps = core.project.fps;
    let fps_txt = if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    };
    let zoom_pct = (core.camera_here().2 * 100.0).round() as i64;
    format!(
        "Frame {}/{} · {} fps · Zoom {}% · {}",
        core.current + 1,
        core.frame_count(),
        fps_txt,
        zoom_pct,
        core.status
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every i18n key the app bar / transport labels resolve
    /// through must exist in the table — otherwise `tr` would
    /// silently render the raw key as the button label.
    #[test]
    fn chrome_label_keys_exist_in_the_table() {
        for key in [
            "save",
            "load",
            "undo",
            "redo",
            "prev_frame",
            "play",
            "pause",
            "next_frame",
            "add_frame",
            "duplicate_frame",
            "delete_frame",
        ] {
            assert!(i18n_wire::key_in_table(key), "label key {key} not in table");
        }
    }

    /// With the default language (English) the wired labels are
    /// exactly the table's English text — the documented
    /// change from the Phase C literals included.
    #[test]
    fn english_labels_match_the_table() {
        let core = EditorCore::new();
        assert_eq!(i18n_wire::tr(&core, "save"), "Save");
        assert_eq!(i18n_wire::tr(&core, "prev_frame"), "Previous frame");
        assert_eq!(i18n_wire::tr(&core, "next_frame"), "Next frame");
        assert_eq!(i18n_wire::tr(&core, "delete_frame"), "Delete");
    }

    /// The Phase N family members construct under both
    /// themes and in both of `btn_muted_live`'s branches.
    /// (Element trees are not readable from unit tests —
    /// construction is the assertable level here; the
    /// Runtime press tests are the behaviour net.)
    #[test]
    fn phase_n_family_members_construct() {
        let dark = tok::theme(true);
        let light = tok::theme(false);
        for t in [&dark, &light] {
            let _ = btn("x-btn", "Btn", false, t, |_| {});
            let _ = btn_inert("x-inert", "Inert", t);
            let _ = btn_muted_live("x-muted", "Muted", false, t, |_| {});
            let _ = btn_muted_live("x-muted-on", "Muted on", true, t, |_| {});
            let _ = danger_btn("x-danger", "Delete", t, |_| {});
        }
    }

    /// The danger constants are the exact deepened reds
    /// P12 verified (≈ 5:1 for white on the resting fill),
    /// and each one round-trips through the shell's hex
    /// conversion to itself — F3's `danger_icon_btn`
    /// imports these values, so a silent drift here would
    /// split the danger family again.
    #[test]
    fn danger_constants_are_the_p12_deepened_reds() {
        assert_eq!(DANGER_FILL, 0xC93A44);
        assert_eq!(DANGER_HOVER, 0xB2323E);
        assert_eq!(DANGER_PRESS, 0x9C2934);
        assert_eq!(danger_color(DANGER_FILL).hex(), "#c93a44ff");
        assert_eq!(danger_color(DANGER_HOVER).hex(), "#b2323eff");
        assert_eq!(danger_color(DANGER_PRESS).hex(), "#9c2934ff");
    }

    /// The app-bar dot is present exactly while the core
    /// reports edits the autosave has not captured — the
    /// status strip's dirty signal, not a side channel.
    #[test]
    fn dirty_dot_follows_the_autosave_edit_counter() {
        let mut core = EditorCore::new();
        for dark in [true, false] {
            let t = app_theme::app_theme(dark);
            assert!(dirty_dot(&core, &t).is_none(), "a fresh core reads clean");
        }
        core.snapshot();
        for dark in [true, false] {
            let t = app_theme::app_theme(dark);
            assert!(
                dirty_dot(&core, &t).is_some(),
                "one snapshot-counted edit lights the dot (dark={dark})"
            );
        }
    }

    /// UI mission Wave 9: the dot paints the app palette's
    /// accent for the theme in hand — `#6C8CFF` in dark,
    /// the darkened `#4666D8` in light — through the
    /// `dirty_dot_fill` seam, never Rune's hard-coded
    /// dark accent.
    #[test]
    fn dirty_dot_fill_is_the_theme_accent() {
        let dark = app_theme::app_theme(true);
        let light = app_theme::app_theme(false);
        assert_eq!(dirty_dot_fill(&dark).hex(), "#6c8cffff");
        assert_eq!(dirty_dot_fill(&light).hex(), "#4666d8ff");
        assert_eq!(dirty_dot_fill(&dark), app_theme::accent(&dark));
        assert_eq!(dirty_dot_fill(&light), app_theme::accent(&light));
    }

    /// UI mission Wave 3 (audit S04 D2): the app bar
    /// constructs in both Undo/Redo gating states — a
    /// fresh core (both stacks empty, both buttons
    /// inert) and after one snapshot-counted edit (Undo
    /// live, Redo still inert). Element trees are not
    /// readable from unit tests, so construction is the
    /// assertable level here; which form renders is the
    /// depth comparison in `app_bar` itself, and the
    /// presented-tree proof belongs to the Runtime suite.
    #[test]
    fn app_bar_builds_with_empty_and_nonempty_undo_stacks() {
        let t = tok::theme(true);
        let mut s = crate::init();
        assert_eq!(s.core.undo_depth(), 0, "fresh core: nothing to undo");
        assert_eq!(s.core.redo_depth(), 0, "fresh core: nothing to redo");
        let _ = app_bar(&s, &t);
        s.core.snapshot();
        assert!(s.core.undo_depth() > 0, "one edit deepens the undo stack");
        assert_eq!(s.core.redo_depth(), 0, "redo stays empty until an undo");
        let _ = app_bar(&s, &t);
    }

    /// Wave 10 (audit E9): the transport row constructs in
    /// both Prev/Next gating states — the populated demo
    /// project (both live) and a zero-frame project (both
    /// inert twins). Element trees are not readable from
    /// unit tests, so construction is the assertable level
    /// here; which form renders is the frame-count
    /// comparison in `transport_row` itself, and the
    /// presented-tree proof belongs to the Runtime suite.
    /// The zero-frame state is reached by clearing the
    /// demo project's frames directly — the same state a
    /// loaded zero-frame project file produces.
    #[test]
    fn transport_row_builds_on_populated_and_empty_projects() {
        let t = tok::theme(true);
        let mut s = crate::init();
        assert!(s.core.frame_count() > 0, "the demo project has frames");
        let _ = transport_row(&s, &t);
        s.core.project.frames.clear();
        assert_eq!(s.core.frame_count(), 0, "the project is now empty");
        let _ = transport_row(&s, &t);
    }

    /// UI mission Wave 4 (audit S16 D2): the app-bar
    /// Load press arms while the session is dirty and
    /// only the confirming press reaches `core.load()`
    /// (whose own status — a load report or its
    /// no-file refusal — replaces the arm prompt). A
    /// clean session's press runs immediately.
    #[test]
    fn load_pressed_arms_first_while_dirty_then_confirms() {
        let mut s = crate::init();
        s.core.snapshot();
        load_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::OpenProject));
        assert!(
            s.core.status.contains("press Load again"),
            "arming explains itself: {}",
            s.core.status
        );
        load_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "the confirming press clears the arm");
        assert!(
            s.core.status.starts_with("Load"),
            "the second press ran the load: {}",
            s.core.status
        );
        // And while clean, a press is the load itself,
        // with no arm in between.
        let mut s = crate::init();
        load_pressed(&mut s);
        assert_eq!(s.confirm_action, None);
        assert!(
            s.core.status.starts_with("Load"),
            "clean press loads immediately: {}",
            s.core.status
        );
    }

    /// Wave P (P4): the legacy timeline chips construct
    /// with their press state in place, on a populated
    /// project under both themes. Construction is the
    /// assertable level (element internals are opaque,
    /// as above); the builder is unmounted, so this pins
    /// the retained census baseline only.
    #[test]
    fn timeline_row_chips_construct_under_both_themes() {
        let s = crate::init();
        assert!(s.core.frame_count() > 0, "the demo project has frames");
        let _ = timeline_row(&s, &tok::theme(true));
        let _ = timeline_row(&s, &tok::theme(false));
    }

    #[test]
    fn status_text_reports_frame_fps_zoom_and_status() {
        let core = EditorCore::new();
        let txt = status_text(&core);
        assert!(txt.starts_with("Frame 1/"), "unexpected status: {txt}");
        assert!(txt.contains(" fps · Zoom 100% · "), "unexpected status: {txt}");
        assert!(txt.ends_with(&core.status), "status op text missing: {txt}");
    }

    #[test]
    fn status_text_formats_fractional_fps() {
        let mut core = EditorCore::new();
        core.project.fps = 29.97;
        assert!(status_text(&core).contains("30.0 fps") || status_text(&core).contains("29.9 fps"));
    }
}
