//! Help panel for the Rune editor shell (Rune migration
//! Phase D, section 25/40): a pure builder over `&EditorCore`
//! — it reads project facts for its header and never mutates
//! the core. The panel's open/close state lives in the shell's
//! menu state, not here.
//!
//! Content discipline (the GPUI-era `ui::help` rule, kept):
//! every shortcut row names a key that does something real.
//! A row survives if either the Rune shell handles the key
//! today (`main.rs` `on_key`, `chrome_view` buttons) or the
//! GPUI-era binding's backing op/field still exists in
//! `editor.rs`. Dropped from the GPUI-era list, deliberately:
//!
//! - `Alt+1` … `Alt+8` (workspace switching) — workspace was
//!   GPUI view state; `EditorCore` has no workspace op, and
//!   the Rune shell has no workspace state yet. The panel
//!   says so in the Workspaces section instead of listing
//!   dead keys.
//! - The nine text-authoring capture rows (`Enter`, `Esc`,
//!   `Backspace`, `Home`, `End`, typing, and the text-mode
//!   `Delete`/`Left`/`Right`) — the engine's `text_edit`
//!   module survives, but the Rune shell does not wire a
//!   text-authoring capture yet, so those keys currently
//!   do nothing. `Delete` still appears once, with its real
//!   shell meaning: delete the selected segment.
//!
//! Search is a later section; the rows are static data
//! ([`shortcut_rows`], [`section_rows`], [`group_rows`]) so a
//! search field can filter them without changing this panel.

use rune::tok::{type_, Family, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;

/// The keyboard shortcuts that are real in this shell, as
/// (key label, action) rows, grouped by section in panel
/// order (see [`group_rows`]). Concatenating [`section_rows`]
/// over the five sections yields exactly this list.
pub fn shortcut_rows() -> Vec<(&'static str, &'static str)> {
    let mut rows = Vec::new();
    for (name, _) in group_rows() {
        rows.extend(section_rows(name));
    }
    rows
}

/// The shortcut rows belonging to one help section, by the
/// section names from [`group_rows`]. Workspaces and Drawing
/// currently have no live keys, so they return empty lists —
/// the panel renders their [`section_note`] text instead.
pub fn section_rows(section: &str) -> Vec<(&'static str, &'static str)> {
    match section {
        "Shortcuts" => vec![
            ("Ctrl+Z", "Undo"),
            ("Ctrl+Shift+Z", "Redo"),
            ("Ctrl+Y", "Redo"),
            ("Delete", "Delete the selected segment"),
        ],
        "Workspaces" => vec![],
        "Posing" => vec![
            ("C", "Toggle camera mode"),
            ("O", "Toggle onion skin"),
        ],
        "Timeline" => vec![
            ("Space", "Play / pause"),
            ("Left", "Previous frame (wraps around)"),
            ("Right", "Next frame (wraps around)"),
            ("N", "Add a frame after the current one"),
            ("I", "Insert an inbetween frame"),
        ],
        "Drawing" => vec![],
        _ => vec![],
    }
}

/// The explanatory note rendered in place of rows for a
/// section that has no live shortcuts yet. `None` for
/// sections that have rows.
pub fn section_note(section: &str) -> Option<&'static str> {
    match section {
        "Workspaces" => Some(
            "Alt+1 to Alt+8 switched workspaces in the GPUI shell. \
             The Rune shell has no workspace state yet, so those keys \
             currently do nothing.",
        ),
        "Drawing" => Some(
            "No keyboard shortcuts yet. Draw mode exists in the core \
             and stage drags paint while it is on, but no key toggles it.",
        ),
        _ => None,
    }
}

/// The help panel's sections, as (name, one-line description)
/// rows, in display order.
pub fn group_rows() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Shortcuts", "General editing keys that work anywhere in the shell"),
        ("Workspaces", "Workspace switching keys"),
        ("Posing", "Keys that change how stage drags behave and what the stage shows"),
        ("Timeline", "Playback and frame keys"),
        ("Drawing", "Freehand drawing keys"),
    ]
}

/// One key cap: a small bordered pill holding the key label
/// in the mono label style.
fn key_cap(key: &str, t: &Theme) -> El {
    el()
        .px(6.0)
        .py(2.0)
        .radius(4.0)
        .fill(t.card)
        .border(1.0, t.control_track)
        .child(
            text(key.to_string())
                .font(type_::LABEL.family(Family::Mono))
                .color(t.text),
        )
}

/// One shortcut row: a key cap, then the action it performs.
fn shortcut_row(key: &str, action: &str, t: &Theme) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(key_cap(key, t))
        .child(text(action.to_string()).font(type_::LABEL).color(t.text))
}

/// One help section: a title, its description, then either
/// its shortcut rows or its explanatory note.
fn help_section(name: &str, description: &str, t: &Theme) -> El {
    let mut section = col()
        .key(format!("help-section-{}", name.to_lowercase()))
        .gap(4.0)
        .child(text(name.to_string()).font(type_::TITLE).color(t.text))
        .child(
            text(description.to_string())
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    let rows = section_rows(name);
    if rows.is_empty() {
        if let Some(note) = section_note(name) {
            section = section.child(
                text(note.to_string())
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
    } else {
        for (key, action) in rows {
            section = section.child(shortcut_row(key, action, t));
        }
    }
    section
}

/// The help panel: a header (title + the open project's
/// name, frame count and fps, read from the core) over the
/// five sections from [`group_rows`]. Pure: it builds
/// elements from `core` and never calls an op on it.
pub fn help_panel(core: &EditorCore, t: &Theme) -> El {
    let fps = core.project.fps;
    let fps_txt = if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    };
    let mut panel = col()
        .key("help-panel")
        .fill(t.card)
        .border(1.0, t.control_track)
        .radius(8.0)
        .padding(12.0)
        .gap(10.0)
        .child(text("Help".to_string()).font(type_::TITLE).color(t.text))
        .child(
            text(format!(
                "{} · {} frames · {} fps",
                core.project.name,
                core.frame_count(),
                fps_txt
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        );
    for (name, description) in group_rows() {
        panel = panel.child(help_section(name, description, t));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::{group_rows, section_note, section_rows, shortcut_rows};

    #[test]
    fn shortcut_rows_has_exactly_11_rows() {
        // 4 Shortcuts + 2 Posing + 5 Timeline; Workspaces and
        // Drawing contribute none (see the module docs for
        // which GPUI-era rows were dropped and why).
        assert_eq!(shortcut_rows().len(), 11);
    }

    #[test]
    fn shortcut_rows_have_no_duplicate_keys() {
        let rows = shortcut_rows();
        for (i, (key, _)) in rows.iter().enumerate() {
            assert!(
                !rows[..i].iter().any(|(k, _)| k == key),
                "duplicate shortcut key: {key}"
            );
        }
    }

    #[test]
    fn shortcut_rows_spot_checks() {
        let rows = shortcut_rows();
        // First row is the undo binding; the shell-handled and
        // surviving GPUI-era bindings are all present.
        assert_eq!(rows[0], ("Ctrl+Z", "Undo"));
        assert!(rows.contains(&("Ctrl+Shift+Z", "Redo")));
        assert!(rows.contains(&("Ctrl+Y", "Redo")));
        assert!(rows.contains(&("Space", "Play / pause")));
        assert!(rows.contains(&("Left", "Previous frame (wraps around)")));
        assert!(rows.contains(&("Right", "Next frame (wraps around)")));
        assert!(rows.contains(&("Delete", "Delete the selected segment")));
        assert!(rows.contains(&("N", "Add a frame after the current one")));
        assert!(rows.contains(&("I", "Insert an inbetween frame")));
        assert!(rows.contains(&("C", "Toggle camera mode")));
        assert!(rows.contains(&("O", "Toggle onion skin")));
    }

    #[test]
    fn dropped_gpui_era_rows_stay_dropped() {
        let rows = shortcut_rows();
        // Workspace switching has no backing op in
        // EditorCore and no state in the Rune shell.
        assert!(
            !rows.iter().any(|(k, _)| k.starts_with("Alt+")),
            "workspace keys must not be listed as live shortcuts"
        );
        // The text-authoring capture is not wired in the
        // Rune shell, so none of its rows may appear.
        assert!(
            !rows.iter().any(|(_, a)| a.starts_with("Text editing:")),
            "text-editing rows must not be listed as live shortcuts"
        );
    }

    #[test]
    fn section_rows_partition_shortcut_rows() {
        let mut joined = Vec::new();
        for (name, _) in group_rows() {
            joined.extend(section_rows(name));
        }
        assert_eq!(joined, shortcut_rows());
    }

    #[test]
    fn empty_sections_carry_notes_and_filled_sections_do_not() {
        assert!(section_rows("Workspaces").is_empty());
        assert!(section_rows("Drawing").is_empty());
        assert!(section_note("Workspaces").is_some());
        assert!(section_note("Drawing").is_some());
        for name in ["Shortcuts", "Posing", "Timeline"] {
            assert!(!section_rows(name).is_empty(), "{name} should have rows");
            assert!(section_note(name).is_none(), "{name} should have no note");
        }
    }

    #[test]
    fn group_rows_covers_all_five_sections_in_order() {
        let rows = group_rows();
        assert_eq!(rows.len(), 5);
        let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            ["Shortcuts", "Workspaces", "Posing", "Timeline", "Drawing"]
        );
    }

    #[test]
    fn group_names_are_unique() {
        let rows = group_rows();
        for (i, (name, _)) in rows.iter().enumerate() {
            assert!(
                !rows[..i].iter().any(|(n, _)| n == name),
                "duplicate section name: {name}"
            );
        }
    }

    #[test]
    fn no_empty_strings_anywhere() {
        for (key, action) in shortcut_rows() {
            assert!(!key.trim().is_empty(), "empty shortcut key");
            assert!(!action.trim().is_empty(), "empty action for {key}");
        }
        for (name, description) in group_rows() {
            assert!(!name.trim().is_empty(), "empty section name");
            assert!(
                !description.trim().is_empty(),
                "empty description for {name}"
            );
        }
    }
}
