//! Help panel for the Rune editor shell (Rune migration
//! Phase D, section 25/40; Phase E, section 23/40): a pure
//! builder over `&EditorCore` — it reads project facts for
//! its header and never mutates the core. The panel's
//! open/close state lives in the shell's menu state, not
//! here.
//!
//! Content discipline (the GPUI-era `ui::help` rule, kept):
//! every shortcut row names a key that does something real.
//! A row survives if the Rune shell handles the key today:
//! the `main.rs` `on_key` bindings, `chrome_view` buttons,
//! the command palette's key handler (which `on_key`
//! consults first), or — since Phase E — the shell's
//! workspace keys. Two groups of GPUI-era rows were dropped
//! in Phase D; Phase E restores one of them:
//!
//! - `Alt+1` … `Alt+8` (workspace switching) — dropped in
//!   Phase D because the Rune shell had no workspace state;
//!   restored in Phase E, where the shell's `on_key` binds
//!   them to the workspaces in table order (Animate, Draw,
//!   Graph, Effects, Audio, 3D, Export, Home). They are
//!   shell bindings, not `EditorCore` ops.
//! - The nine text-authoring capture rows (`Enter`, `Esc`,
//!   `Backspace`, `Home`, `End`, typing, and the text-mode
//!   `Delete`/`Left`/`Right`) — still dropped: the engine's
//!   `text_edit` module survives, but the Rune shell does
//!   not wire a text-authoring capture, so in that sense
//!   those keys do nothing. `Delete` still appears once,
//!   with its real shell meaning: delete the selected
//!   segment. (The guides toggle is a button in the View
//!   menu, not a key, so it is not a row either.)
//!
//! Search is a later section; the rows are static data
//! ([`shortcut_rows`], [`section_rows`], [`group_rows`]) so a
//! search field can filter them without changing this panel.
//!
//! Phase G (section 30/40) audit, 2026-11-08: every binding
//! below was re-enumerated from source — `main.rs` `on_key`
//! read in full, `command_palette::palette_handle_key` and
//! `palette_field_key`, plus a tree-wide grep for
//! `KeyEvent` / `is_command` consumers (only those two
//! handlers exist; `menus.rs` mentions keys only in doc
//! comments for its shortcut hints, and `graph_panel`'s
//! `Key::` is graph keyframe data, not keyboard input).
//! The audit added the palette-open rows (`Down`, `Up`,
//! `Enter`) and the `Escape` row (the palette handler
//! closes the palette; `on_key` closes an open menu), and
//! recorded the `text_focused` guard in the group
//! descriptions: `on_key` consults the palette handler
//! first, then returns early while any text field holds
//! focus — so every shell binding except Ctrl+K pauses
//! while typing. Field-local Enter (rename and name fields
//! applying on submit) stays excluded, like the other
//! text-authoring keys above.

use rune::tok::{type_, Family, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::icons::icon_el;

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
/// section names from [`group_rows`]. Drawing currently has
/// no live keys, so it returns an empty list — the panel
/// renders its [`section_note`] text instead.
pub fn section_rows(section: &str) -> Vec<(&'static str, &'static str)> {
    match section {
        "Shortcuts" => vec![
            ("Ctrl+Z", "Undo"),
            ("Ctrl+Shift+Z", "Redo"),
            ("Ctrl+Y", "Redo"),
            ("Ctrl+K", "Open the command palette"),
            ("Down", "Next result in the command palette (palette open)"),
            ("Up", "Previous result in the command palette (palette open)"),
            ("Enter", "Run the selected command (command palette open)"),
            ("Escape", "Close the open menu or command palette"),
            ("Delete", "Delete the selected segment"),
        ],
        "Workspaces" => vec![
            ("Alt+1", "Switch to the Animate workspace"),
            ("Alt+2", "Switch to the Draw workspace"),
            ("Alt+3", "Switch to the Graph workspace"),
            ("Alt+4", "Switch to the Effects workspace"),
            ("Alt+5", "Switch to the Audio workspace"),
            ("Alt+6", "Switch to the 3D workspace"),
            ("Alt+7", "Switch to the Export workspace"),
            ("Alt+8", "Switch to the Home workspace"),
        ],
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
        (
            "Shortcuts",
            "General editing keys — all but Ctrl+K pause while typing in a text field",
        ),
        (
            "Workspaces",
            "Workspace switching keys — paused while typing in a text field",
        ),
        (
            "Posing",
            "Keys that change how stage drags behave and what the stage shows — paused while typing in a text field",
        ),
        (
            "Timeline",
            "Playback and frame keys — paused while typing in a text field",
        ),
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
/// its shortcut rows or its explanatory note. The title
/// carries a small glyph (Phase H): `key` for the Shortcuts
/// section, `list` for the rest — decoration only; the
/// section names and descriptions are untouched.
fn help_section(name: &str, description: &str, t: &Theme) -> El {
    let header_icon = if name == "Shortcuts" { "key" } else { "list" };
    let mut section = col()
        .key(format!("help-section-{}", name.to_lowercase()))
        .gap(4.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el(header_icon, 12.0, t.text))
                .child(text(name.to_string()).font(type_::TITLE).color(t.text)),
        )
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
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon_el("circle-question", 14.0, t.text))
                .child(text("Help".to_string()).font(type_::TITLE).color(t.text)),
        )
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
    fn shortcut_rows_has_exactly_24_rows() {
        // 9 Shortcuts + 8 Workspaces + 2 Posing + 5 Timeline;
        // Drawing contributes none (see the module docs for
        // which GPUI-era rows were dropped and why). Phase G
        // added the four palette/menu rows to Shortcuts.
        assert_eq!(shortcut_rows().len(), 24);
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
        assert!(rows.contains(&("Ctrl+K", "Open the command palette")));
        assert!(rows.contains(&("Alt+1", "Switch to the Animate workspace")));
        assert!(rows.contains(&("Alt+8", "Switch to the Home workspace")));
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
    fn workspace_rows_cover_all_eight_workspaces_in_table_order() {
        // Phase E wires Alt+1..Alt+8 in the shell's on_key,
        // in the workspaces table order; the section lists
        // exactly those eight rows, in that order.
        let expected: Vec<(&str, &str)> = vec![
            ("Alt+1", "Switch to the Animate workspace"),
            ("Alt+2", "Switch to the Draw workspace"),
            ("Alt+3", "Switch to the Graph workspace"),
            ("Alt+4", "Switch to the Effects workspace"),
            ("Alt+5", "Switch to the Audio workspace"),
            ("Alt+6", "Switch to the 3D workspace"),
            ("Alt+7", "Switch to the Export workspace"),
            ("Alt+8", "Switch to the Home workspace"),
        ];
        assert_eq!(section_rows("Workspaces"), expected);
    }

    #[test]
    fn phase_g_audit_rows_are_listed() {
        // The bindings the Phase G audit found in
        // `command_palette::palette_handle_key` (consulted
        // by `on_key` before anything else) and in
        // `on_key`'s menu branch — all real, none listed
        // before Phase G.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Down",
            "Next result in the command palette (palette open)"
        )));
        assert!(rows.contains(&(
            "Up",
            "Previous result in the command palette (palette open)"
        )));
        assert!(rows.contains(&(
            "Enter",
            "Run the selected command (command palette open)"
        )));
        assert!(rows.contains(&("Escape", "Close the open menu or command palette")));
    }

    #[test]
    fn group_descriptions_state_the_typing_guard() {
        // `on_key` returns early while a text field holds
        // focus, after the palette handler has had first
        // claim — so every group whose keys are shell
        // bindings must say they pause while typing, and
        // Shortcuts must name the one exception the routing
        // order creates (Ctrl+K reaches the palette handler
        // before the guard). Drawing has no keys to guard.
        for (name, description) in group_rows() {
            if name == "Drawing" {
                continue;
            }
            assert!(
                description.contains("typing"),
                "{name} description must state the typing guard: {description}"
            );
        }
        let shortcuts = group_rows()[0].1;
        assert!(
            shortcuts.contains("Ctrl+K"),
            "Shortcuts must name the Ctrl+K exception: {shortcuts}"
        );
    }

    #[test]
    fn dropped_gpui_era_rows_stay_dropped() {
        let rows = shortcut_rows();
        // The text-authoring capture is not wired in the
        // Rune shell, so none of its rows may appear.
        // (Workspace keys, dropped in Phase D, are live
        // again since Phase E — see the workspace test.)
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
        assert!(section_rows("Drawing").is_empty());
        assert!(section_note("Drawing").is_some());
        for name in ["Shortcuts", "Workspaces", "Posing", "Timeline"] {
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
